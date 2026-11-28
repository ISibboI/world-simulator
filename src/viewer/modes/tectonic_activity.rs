//! Tectonic activity (#110): where earthquakes and volcanism are expected, as long-term rates
//! that the simulation estimates from its plate boundaries, deformation, slabs and plumes (see
//! [`crate::tectonics::Simulation::activity`] and `notes/issue-110/plan.md`).
//!
//! Earthquakes are shown as the seismic moment rate per area on a logarithmic scale, volcanism
//! as the magma supply per area (intrusions included) on its own, eruptions as its erupted
//! share (one decade lower on the same colours); "Both" shows the earthquakes with volcano
//! markers at the arcs, hotspots, rifts and large igneous provinces (ridges are left out of the
//! markers:
//! they would line every ridge, and the earthquake colours show them already). Everything comes
//! from the cells every frame keeps, so past frames show alike.

use glam::DVec3;

use super::plates::{boundaries, boundary_swatches};
use super::{
    ColorScale, Legend, ModeData, ModeOption, OptionKind, OptionValues, RenderMode, Shading,
    Swatch, SwatchShape, Text, coastlines, rgba,
};
use crate::tectonics::VolcanicSource;
use crate::viewer::CellInfo;
use crate::viewer::overlay::{OverlayBuilder, thin};

pub struct TectonicActivity;

/// What the mode shows: the value of the option [`SHOW`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Earthquakes,
    Volcanism,
    Eruptions,
    Both,
}

impl Shown {
    fn of(options: &OptionValues) -> Self {
        match options.get(SHOW) {
            1 => Shown::Volcanism,
            2 => Shown::Eruptions,
            3 => Shown::Both,
            _ => Shown::Earthquakes,
        }
    }
}

/// Indices of the options.
const SHOW: usize = 0;
const BOUNDARIES: usize = 1;

const OPTIONS: [ModeOption; 2] = [
    ModeOption {
        label: "Show",
        hover: "Earthquakes (the seismic moment rate), volcanism (the magma supply, \
                intrusions included), eruptions (the share of it that erupts), or both \
                (earthquakes with volcano markers)",
        kind: OptionKind::Choice(&["Earthquakes", "Volcanism", "Eruptions", "Both"]),
    },
    ModeOption {
        label: "Plate boundaries",
        hover: "The plate boundaries by what they do, as in the Plates mode",
        kind: OptionKind::Switch { default: false },
    },
];

/// The range of the seismic scale: log₁₀ of the moment rate in N·m yr⁻¹ km⁻² (the stops span
/// it).
const SEISMIC_LOW: f64 = 10.0;
const SEISMIC_HIGH: f64 = 16.0;
/// Yellow to dark red (ColorBrewer YlOrRd, sequential, colour-blind safe), by decade.
const SEISMIC_STOPS: [(f64, [u8; 3]); 7] = [
    (10.0, [255, 237, 160]),
    (11.0, [254, 210, 110]),
    (12.0, [253, 160, 70]),
    (13.0, [245, 100, 45]),
    (14.0, [215, 35, 35]),
    (15.0, [155, 0, 38]),
    (16.0, [80, 0, 30]),
];

/// The bottom of the volcanic scale: log₁₀ of the magma supply in mm/yr; the eruption scale is
/// a decade lower (a tenth to a half of the magma erupts).
const MAGMA_LOW: f64 = -3.0;
const ERUPTION_OFFSET: f64 = -1.0;
/// Pale lilac to dark magenta (ColorBrewer PuRd), apart from the earthquake colours.
const MAGMA_STOPS: [(f64, [u8; 3]); 5] = [
    (-3.0, [236, 226, 240]),
    (-2.0, [212, 170, 214]),
    (-1.0, [220, 110, 180]),
    (0.0, [206, 18, 86]),
    (1.0, [103, 0, 31]),
];

/// Colours of cells without activity on the scales: grey-blue ocean, grey land, so that the
/// geography stays readable.
const QUIET_OCEAN: [u8; 3] = [150, 162, 178];
const QUIET_LAND: [u8; 3] = [196, 192, 184];

/// Volcano markers ("Both"): at cells with at least this magma supply (mm/yr) from an arc,
/// hotspot or rift, at least this many arrow spacings apart, of this size (arrow spacings).
const MARKER_MIN_SUPPLY: f32 = 0.01;
const MARKER_SPACING: f64 = 0.6;
const MARKER_SIZE: f64 = 0.22;
const MARKER_BORDER: [f32; 4] = [0.95, 0.95, 0.95, 1.0];

/// Colours of the volcanic sources (Okabe–Ito, colour-blind safe; dark enough for the
/// earthquake colours).
fn volcanic_color(source: VolcanicSource) -> [u8; 3] {
    match source {
        VolcanicSource::Arc => [30, 30, 30],
        VolcanicSource::Ridge => [86, 180, 233],
        VolcanicSource::Hotspot => [0, 114, 178],
        VolcanicSource::Rift => [0, 158, 115],
        VolcanicSource::Lip => [230, 159, 0],
    }
}

/// Lines: widths in multiples of the arrow spacing, colours and lift (planet radii).
const COAST_WIDTH: f64 = 0.03;
const COAST_COLOR: [u8; 3] = [70, 70, 75];
const BOUNDARY_SCALE: f64 = 0.6;
const LIFT: f64 = 0.003;

/// "10ⁿ" with a superscript exponent.
pub fn power_of_ten(exponent: i32) -> String {
    const DIGITS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    let mut text = String::from("10");
    if exponent < 0 {
        text.push('⁻');
    }
    for digit in exponent.unsigned_abs().to_string().bytes() {
        text.push(DIGITS[usize::from(digit - b'0')]);
    }
    text
}

fn seismic_scale() -> ColorScale {
    ColorScale {
        stops: SEISMIC_STOPS.to_vec(),
        title: "Seismic moment rate (N·m per year and km²)".into(),
        ticks: (SEISMIC_LOW as i32..=SEISMIC_HIGH as i32)
            .step_by(2)
            .map(|exponent| (f64::from(exponent), power_of_ten(exponent).into()))
            .collect(),
    }
}

fn magma_scale() -> ColorScale {
    ColorScale {
        stops: MAGMA_STOPS.to_vec(),
        title: "Magma supply (mm per year, as a layer)".into(),
        ticks: vec![
            (-3.0, "0.001".into()),
            (-2.0, "0.01".into()),
            (-1.0, "0.1".into()),
            (0.0, "1".into()),
            (1.0, "10".into()),
        ],
    }
}

fn eruption_scale() -> ColorScale {
    ColorScale {
        stops: MAGMA_STOPS
            .iter()
            .map(|&(value, color)| (value + ERUPTION_OFFSET, color))
            .collect(),
        title: "Eruption rate (mm per year, as a layer)".into(),
        ticks: vec![
            (-4.0, "0.0001".into()),
            (-3.0, "0.001".into()),
            (-2.0, "0.01".into()),
            (-1.0, "0.1".into()),
            (0.0, "1".into()),
        ],
    }
}

/// The colour of a cell for a rate on a logarithmic scale from `10^low`: the quiet colour
/// below it.
fn color(scale: &ColorScale, value: f32, low: f64, cell: &CellInfo) -> [u8; 3] {
    if value.is_finite() && value > 0.0 && f64::from(value).log10() >= low {
        scale.color(f64::from(value).log10()).unwrap_or(QUIET_LAND)
    } else if cell.is_sea() {
        QUIET_OCEAN
    } else {
        QUIET_LAND
    }
}

/// Draws a volcano marker: a triangle pointing north of `size` (planet radii) around `center`.
fn volcano(overlay: &mut OverlayBuilder, center: DVec3, size: f64, color: [f32; 4]) {
    let up = center.normalize_or_zero();
    if up == DVec3::ZERO {
        return;
    }
    let east = DVec3::Z.cross(up).try_normalize().unwrap_or(DVec3::X);
    let north = up.cross(east);
    let corner = |x: f64, y: f64| center + (east * x + north * y) * size;
    overlay.triangle(
        [corner(0.0, 1.0), corner(-0.866, -0.5), corner(0.866, -0.5)],
        color,
        0.12 * size,
        MARKER_BORDER,
    );
}

impl RenderMode for TectonicActivity {
    fn label(&self) -> &'static str {
        "Tectonic activity"
    }

    fn options(&self) -> &'static [ModeOption] {
        &OPTIONS
    }

    fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
        let shown = Shown::of(data.options);
        let (scale, low) = match shown {
            Shown::Volcanism => (magma_scale(), MAGMA_LOW),
            Shown::Eruptions => (eruption_scale(), MAGMA_LOW + ERUPTION_OFFSET),
            Shown::Earthquakes | Shown::Both => (seismic_scale(), SEISMIC_LOW),
        };
        Some(
            data.cells
                .iter()
                .map(|cell| {
                    let value = match shown {
                        Shown::Volcanism => cell.magma_supply,
                        Shown::Eruptions => cell.eruptions,
                        Shown::Earthquakes | Shown::Both => cell.seismicity,
                    };
                    rgba(color(&scale, value, low, cell))
                })
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
        coastlines(
            data,
            overlay,
            COAST_WIDTH * spacing,
            rgba(COAST_COLOR),
            LIFT,
        );
        overlay.next_layer();
        if data.options.is_on(BOUNDARIES) {
            boundaries(data, overlay, BOUNDARY_SCALE);
            overlay.next_layer();
        }
        if Shown::of(data.options) == Shown::Both {
            // The strongest first, so that thinning keeps them.
            let mut volcanoes: Vec<(u32, VolcanicSource)> = data
                .cells
                .iter()
                .enumerate()
                .filter_map(|(cell, info)| {
                    let source = info.volcanic_source()?;
                    (source != VolcanicSource::Ridge && info.magma_supply >= MARKER_MIN_SUPPLY)
                        .then_some((cell as u32, source))
                })
                .collect();
            volcanoes.sort_by(|a, b| {
                data.cells[b.0 as usize]
                    .magma_supply
                    .total_cmp(&data.cells[a.0 as usize].magma_supply)
                    .then(a.0.cmp(&b.0))
            });
            let position = |cell: u32| {
                let point = data.position(cell).as_dvec3();
                point + point.normalize_or_zero() * LIFT * 2.0
            };
            let kept = thin(
                volcanoes.iter().map(|&(cell, _)| position(cell)),
                MARKER_SPACING * spacing,
            );
            for index in kept {
                let (cell, source) = volcanoes[index];
                volcano(
                    overlay,
                    position(cell),
                    MARKER_SIZE * spacing,
                    rgba(volcanic_color(source)),
                );
            }
        }
    }

    fn legend(&self, options: &OptionValues, _data: Option<&ModeData>) -> Legend {
        let shown = Shown::of(options);
        let mut swatches = Vec::new();
        match shown {
            Shown::Earthquakes | Shown::Both => {
                swatches.push(Swatch::fill(QUIET_OCEAN, "Quiet ocean"));
                swatches.push(Swatch::fill(QUIET_LAND, "Quiet land"));
            }
            Shown::Volcanism => {
                swatches.push(Swatch::fill(QUIET_OCEAN, "No magma, ocean"));
                swatches.push(Swatch::fill(QUIET_LAND, "No magma, land"));
            }
            Shown::Eruptions => {
                swatches.push(Swatch::fill(QUIET_OCEAN, "No eruptions, ocean"));
                swatches.push(Swatch::fill(QUIET_LAND, "No eruptions, land"));
            }
        }
        if shown == Shown::Both {
            for (source, label) in [
                (VolcanicSource::Arc, "Arc volcanoes"),
                (VolcanicSource::Hotspot, "Hotspot"),
                (VolcanicSource::Rift, "Rift volcanism"),
                (VolcanicSource::Lip, "Large igneous province"),
            ] {
                swatches.push(Swatch::new(
                    volcanic_color(source),
                    SwatchShape::Triangle,
                    label,
                ));
            }
        }
        swatches.push(Swatch::new(COAST_COLOR, SwatchShape::Line, "Coastline"));
        if options.is_on(BOUNDARIES) {
            swatches.extend(boundary_swatches());
        }
        let description: Text = match shown {
            Shown::Earthquakes => "Where earthquakes are expected: the long-term seismic \
                                   moment rate, estimated from how fast the plates move at \
                                   their boundaries (subduction zones release the most, then \
                                   collision belts and transforms; ridges little) and how fast \
                                   the lithosphere deforms inside the plates. Rates over \
                                   millennia, not single earthquakes."
                .into(),
            Shown::Volcanism => "Where magma rises: the long-term supply of magma (intrusions \
                                 included), from spreading ridges, arcs above the slabs, \
                                 hotspots above mantle plumes, continental rifts and large \
                                 igneous provinces during their pulse of a few Myr. A layer of \
                                 1 mm per year is 1000 m³ per year and km²."
                .into(),
            Shown::Eruptions => "Where lava erupts: the share of the magma supply that reaches \
                                 the surface — about a seventh at ridges (most of it unseen on \
                                 the sea floor), a sixth at arcs, a third at oceanic hotspots, a \
                                 tenth in continents and half in large igneous provinces; the \
                                 rest freezes at depth."
                .into(),
            Shown::Both => "Earthquakes as colours (the long-term seismic moment rate) and \
                            volcanoes as markers: arcs, hotspots, rifts and large igneous \
                            provinces with at least 0.01 mm/yr of magma; spreading ridges, \
                            also volcanic, show in the colours."
                .into(),
        };
        Legend {
            description,
            scale: Some(match shown {
                Shown::Volcanism => magma_scale(),
                Shown::Eruptions => eruption_scale(),
                Shown::Earthquakes | Shown::Both => seismic_scale(),
            }),
            swatches,
            arrows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponents_are_superscript() {
        assert_eq!(power_of_ten(15), "10¹⁵");
        assert_eq!(power_of_ten(-3), "10⁻³");
        assert_eq!(power_of_ten(0), "10⁰");
    }

    #[test]
    fn quiet_cells_keep_land_and_sea_and_active_ones_follow_the_scale() {
        let scale = seismic_scale();
        let sea = CellInfo {
            elevation: -3000.0,
            drainage: CellInfo::OCEAN,
            ..bytemuck::Zeroable::zeroed()
        };
        let land = CellInfo {
            elevation: 500.0,
            ..bytemuck::Zeroable::zeroed()
        };
        assert_eq!(color(&scale, 0.0, SEISMIC_LOW, &sea), QUIET_OCEAN);
        assert_eq!(color(&scale, 1e5, SEISMIC_LOW, &land), QUIET_LAND);
        assert_eq!(color(&scale, f32::NAN, SEISMIC_LOW, &land), QUIET_LAND);
        assert_eq!(color(&scale, 1e15, SEISMIC_LOW, &land), [155, 0, 38]);
        // Darker with more moment.
        let lightness = |value: f32| {
            let [r, g, b] = color(&scale, value, SEISMIC_LOW, &land).map(f64::from);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        };
        assert!(lightness(1e11) > lightness(1e13) && lightness(1e13) > lightness(1e15));
    }
}
