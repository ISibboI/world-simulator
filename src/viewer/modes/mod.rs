//! Render modes of the 3D view: what the planet's colours and overlays show.
//!
//! A mode is a [`RenderMode`]: it names the extra [`Layers`] it needs from the simulation, may
//! colour the cells ([`RenderMode::colors`], from the [`CellInfo`]s and [`PlateInfo`]s every
//! update carries and the requested layers), may draw over the planet ([`RenderMode::overlay`]:
//! arrows, lines, markers, see [`super::overlay`]; vector modes just give
//! [`RenderMode::arrows`]), and describes itself in a [`Legend`]. The topographic mode, the default, keeps the colours the
//! view builds the vertices with (`render::surface_color`), so it looks exactly as before modes
//! existed.
//!
//! The active mode is chosen in the mode switcher ([`switcher`]), on the command line
//! (`--render-mode`) or in the URL of the web version (`?render-mode=…`); the web version also
//! remembers it in the local storage (URL > stored > default, see [`initial`]).
//!
//! Adding a mode: a variant of [`RenderModeId`] (its clap name is the name in the command line
//! and the URL), a type implementing [`RenderMode`] in its own file, and its lines in
//! [`RenderModeId::mode`] and [`RenderModeId::name`]. A mode that needs data beyond
//! [`CellInfo`] adds a [`super::fields::Layer`]. A mode with settings of its own (which month,
//! which lines) lists them in [`RenderMode::options`]; the switcher shows them above the
//! legend and the mode reads their values from [`ModeData::options`].

mod climate;
mod crust;
mod crust_age;
mod hydrology;
mod moisture;
mod ocean_currents;
mod plate_motion;
mod plates;
pub mod realistic;
mod sediment;
mod strength;
pub mod switcher;
mod temperature;
mod topographic;
mod wind;

pub use sediment::bulk as bulk_sediment;

use clap::ValueEnum;
use glam::Vec3;

use super::arrows::{ArrowStyle, Sample, base_point, tangent_vector};
use super::fields::{Fields, Layer, Layers};
use super::overlay::{LineStyle, OverlayBuilder, category_boundaries};
use super::plate_colors::PlateColors;
use super::renderer::Vertex;
use super::{CellInfo, PlateInfo};
use crate::sphere::SphereMesh;

/// The render modes, by their names on the command line and in the URL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum RenderModeId {
    /// The planet in space, lit by the sun, with the stars (for now with the topographic
    /// colours).
    Realistic,
    /// Elevation colours with rivers and permanent ice.
    #[default]
    Topographic,
    /// Plate velocities as arrows over the topographic colours.
    PlateMotion,
    /// Plates in their colours, their boundaries by type, and their motion.
    Plates,
    /// Surface temperature: annual mean, January or July.
    Temperature,
    /// Precipitation (annual, January or July) or the aridity index of land.
    Moisture,
    /// The wind-driven ocean gyres as arrows.
    OceanCurrents,
    /// The prevailing low-level winds as arrows (annual, January or July).
    Wind,
    /// Age of the ocean floor with isochrons; continents in grey.
    CrustAge,
    /// Crust type (oceanic, transitional, continental) and thickness.
    Crust,
    /// Sediment thickness on land and sea floor (logarithmic), with optional isopachs.
    Sediment,
    /// Lithosphere strength: weak zones and what weakens them.
    LithosphereStrength,
    /// Rivers, drainage basins, endorheic areas and lakes.
    Hydrology,
    /// Köppen–Geiger climate classes of land.
    Climate,
}

impl RenderModeId {
    /// The mode's behaviour.
    pub fn mode(self) -> &'static dyn RenderMode {
        match self {
            RenderModeId::Realistic => &realistic::Realistic,
            RenderModeId::Topographic => &topographic::Topographic,
            RenderModeId::PlateMotion => &plate_motion::PlateMotion,
            RenderModeId::Plates => &plates::Plates,
            RenderModeId::Temperature => &temperature::Temperature,
            RenderModeId::Moisture => &moisture::Moisture,
            RenderModeId::OceanCurrents => &ocean_currents::OceanCurrents,
            RenderModeId::Wind => &wind::Wind,
            RenderModeId::CrustAge => &crust_age::CrustAge,
            RenderModeId::Crust => &crust::Crust,
            RenderModeId::Sediment => &sediment::Sediment,
            RenderModeId::LithosphereStrength => &strength::LithosphereStrength,
            RenderModeId::Hydrology => &hydrology::Hydrology,
            RenderModeId::Climate => &climate::Climate,
        }
    }

    /// The layers the mode needs beyond [`CellInfo`].
    pub fn layers(self) -> Layers {
        self.mode().layers()
    }

    /// Every mode, in the order of the switcher.
    pub fn all() -> &'static [RenderModeId] {
        Self::value_variants()
    }

    /// The name on the command line, in the URL and in the local storage (clap's name of the
    /// variant, which a test checks; clap only lends its names).
    pub fn name(self) -> &'static str {
        match self {
            RenderModeId::Realistic => "realistic",
            RenderModeId::Topographic => "topographic",
            RenderModeId::PlateMotion => "plate-motion",
            RenderModeId::Plates => "plates",
            RenderModeId::Temperature => "temperature",
            RenderModeId::Moisture => "moisture",
            RenderModeId::OceanCurrents => "ocean-currents",
            RenderModeId::Wind => "wind",
            RenderModeId::CrustAge => "crust-age",
            RenderModeId::Crust => "crust",
            RenderModeId::Sediment => "sediment",
            RenderModeId::LithosphereStrength => "lithosphere-strength",
            RenderModeId::Hydrology => "hydrology",
            RenderModeId::Climate => "climate",
        }
    }

    /// The mode of a name, if there is one.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Used by the web version.
    pub fn parse(name: &str) -> Option<Self> {
        Self::from_str(name.trim(), false).ok()
    }
}

/// What a mode gets to colour the planet and to place its arrows.
pub struct ModeData<'a> {
    /// The mesh with the directions of the cells.
    pub mesh: &'a SphereMesh,
    /// The displayed vertices of the cells (with the topographic colours).
    pub vertices: &'a [Vertex],
    /// The attributes of the cells.
    pub cells: &'a [CellInfo],
    /// The plates, by index ([`CellInfo::plate`]), and their colours.
    pub plates: &'a [PlateInfo],
    pub plate_colors: &'a PlateColors,
    /// The requested layers, as far as they arrived.
    pub fields: &'a Fields,
    /// Evenly spread cells for arrows, and their spacing in planet radii.
    pub arrow_cells: &'a [u32],
    pub arrow_spacing: f64,
    /// The values of the mode's [`RenderMode::options`].
    pub options: &'a OptionValues,
    /// Whether the planet has no life (`--biosphere abiotic`): no vegetation.
    pub abiotic: bool,
}

impl ModeData<'_> {
    /// The displayed position of a vertex.
    pub fn position(&self, vertex: u32) -> Vec3 {
        self.vertices
            .get(vertex as usize)
            .map_or(Vec3::ZERO, |vertex| Vec3::from(vertex.position))
    }

    /// Arrows of a 3-component layer at [`ModeData::arrow_cells`], or `None` while the layer has
    /// not arrived.
    pub fn vector_samples(&self, layer: Layer) -> Option<Vec<Sample>> {
        self.fields.get(layer)?;
        Some(
            self.arrow_cells
                .iter()
                .filter_map(|&cell| {
                    let vector = self.fields.vector(layer, cell)?;
                    Some(Sample {
                        // Arrows are at most about one spacing long.
                        base: base_point(self.mesh, cell, self.arrow_spacing, |vertex| {
                            self.position(vertex)
                        }),
                        vector: vector.as_dvec3(),
                    })
                })
                .collect(),
        )
    }
}

impl ModeData<'_> {
    /// Arrows of a per-cell field given in local components at `cells` (e.g. from
    /// [`super::arrows::sample_cells`]) `spacing` apart, with the cell of each: `vector` gives
    /// the eastward and northward components of a cell in the unit of the arrows, or `None`
    /// where the field has no value. Zero vectors are left out; nothing while the cells do not
    /// match the mesh.
    pub fn tangent_samples(
        &self,
        cells: &[u32],
        spacing: f64,
        vector: impl Fn(&CellInfo) -> Option<[f32; 2]>,
    ) -> Vec<(u32, Sample)> {
        if self.cells.len() != self.mesh.len() {
            return Vec::new();
        }
        cells
            .iter()
            .filter_map(|&cell| {
                let components = vector(&self.cells[cell as usize])?;
                let vector = tangent_vector(self.mesh.vertex(cell), components);
                vector.try_normalize()?;
                Some((
                    cell,
                    Sample {
                        base: base_point(self.mesh, cell, spacing, |vertex| self.position(vertex)),
                        vector,
                    },
                ))
            })
            .collect()
    }
}

/// A render mode. Everything but the legend is optional.
pub trait RenderMode: Sync {
    /// The name in the switcher.
    fn label(&self) -> &'static str;

    /// The layers the mode needs beyond [`CellInfo`].
    fn layers(&self) -> Layers {
        Layers::NONE
    }

    /// The colour of every cell (sRGB with alpha, 0 to 1, like [`Vertex::color`]), or `None`
    /// to keep the topographic colours.
    fn colors(&self, _data: &ModeData) -> Option<Vec<[f32; 4]>> {
        None
    }

    /// Arrows to draw over the planet with their style, if any (drawn by the default
    /// [`RenderMode::overlay`]).
    fn arrows(&self, _data: &ModeData) -> Option<(Vec<Sample>, ArrowStyle)> {
        None
    }

    /// Draws over the planet: by default the [`RenderMode::arrows`].
    fn overlay(&self, data: &ModeData, overlay: &mut OverlayBuilder) {
        if let Some((samples, style)) = self.arrows(data) {
            overlay.arrows(&samples, &style);
        }
    }

    /// Whether the planet is shaded by the light (relief visible) or shows the colours as they
    /// are (so that they match the legend exactly).
    fn shading(&self) -> Shading {
        Shading::Lit
    }

    /// Settings of the mode, shown above its legend (see [`ModeOption`]).
    fn options(&self) -> &'static [ModeOption] {
        &[]
    }

    /// The legend for the values of the [`RenderMode::options`], from the data of the
    /// displayed state if there is one (ranges, counts and the arrow spacing can follow the
    /// data); rebuilt with every update.
    fn legend(&self, options: &OptionValues, data: Option<&ModeData>) -> Legend;
}

/// A setting of a render mode (see [`RenderMode::options`]), e.g. which month a mode shows or
/// whether it draws some lines. Its value is a small number: the index of the choice, or 0
/// and 1 for a switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeOption {
    /// The text next to the control.
    pub label: &'static str,
    /// What the setting does, shown when the pointer rests on it.
    pub hover: &'static str,
    pub kind: OptionKind,
}

/// The kind of a [`ModeOption`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionKind {
    /// One of these choices (shown as a row of buttons); the first is the default.
    Choice(&'static [&'static str]),
    /// On or off.
    Switch { default: bool },
}

impl OptionKind {
    fn default_value(self) -> u8 {
        match self {
            OptionKind::Choice(_) => 0,
            OptionKind::Switch { default } => u8::from(default),
        }
    }
}

/// The values of a mode's options, in the order of [`RenderMode::options`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionValues(Vec<u8>);

impl OptionValues {
    /// The defaults of `options`.
    pub fn defaults(options: &[ModeOption]) -> Self {
        Self(
            options
                .iter()
                .map(|option| option.kind.default_value())
                .collect(),
        )
    }

    /// The value of the option at `index` (0 if there is none).
    pub fn get(&self, index: usize) -> u8 {
        self.0.get(index).copied().unwrap_or(0)
    }

    /// Whether the switch at `index` is on.
    pub fn is_on(&self, index: usize) -> bool {
        self.get(index) != 0
    }

    pub fn set(&mut self, index: usize, value: u8) {
        if let Some(slot) = self.0.get_mut(index) {
            *slot = value;
        }
    }
}

/// Which part of the year a mode shows: the year (annual means or totals), January (northern
/// winter) or July (northern summer); the value of a [`OptionKind::Choice`] option of
/// [`Period::CHOICES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Annual,
    January,
    July,
}

impl Period {
    /// The choices of an option for the period, in the order of the values.
    pub const CHOICES: &'static [&'static str] = &["Year", "January", "July"];

    /// The period chosen in the option at `index`.
    pub fn of(options: &OptionValues, index: usize) -> Self {
        match options.get(index) {
            1 => Period::January,
            2 => Period::July,
            _ => Period::Annual,
        }
    }

    /// The surface temperature of a cell in °C.
    pub fn temperature(self, cell: &CellInfo) -> f32 {
        match self {
            Period::Annual => cell.temperature,
            Period::January => cell.temperature_january,
            Period::July => cell.temperature_july,
        }
    }

    /// The precipitation of a cell in m/yr.
    pub fn precipitation(self, cell: &CellInfo) -> f32 {
        match self {
            Period::Annual => cell.precipitation,
            Period::January => cell.precipitation_january,
            Period::July => cell.precipitation_july,
        }
    }

    /// The evaporation (of the ocean, evapotranspiration of land) of a cell in m/yr.
    pub fn evaporation(self, cell: &CellInfo) -> f32 {
        match self {
            Period::Annual => cell.evaporation,
            Period::January => cell.evaporation_january,
            Period::July => cell.evaporation_july,
        }
    }

    /// Precipitation minus evaporation of a cell in m/yr.
    pub fn balance(self, cell: &CellInfo) -> f32 {
        self.precipitation(cell) - self.evaporation(cell)
    }

    /// The low-level wind of a cell (eastward, northward) in m/s.
    pub fn wind(self, cell: &CellInfo) -> [f32; 2] {
        match self {
            Period::Annual => [cell.wind_east, cell.wind_north],
            Period::January => [cell.wind_january_east, cell.wind_january_north],
            Period::July => [cell.wind_july_east, cell.wind_july_north],
        }
    }
}

/// How the planet's colours are lit, see [`RenderMode::shading`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shading {
    /// Diffuse light: from the camera, or in the realistic mode as its light option chooses
    /// (see `super::lighting`).
    #[default]
    Lit,
    /// The colours as they are.
    Unlit,
}

/// Spacing of the arrows (planet radii) assumed before the first surface arrived.
pub const DEFAULT_ARROW_SPACING: f64 = 0.14;

/// Text of a legend: fixed, or built from the data.
pub type Text = std::borrow::Cow<'static, str>;

/// What the legend of a mode shows; drawn by [`switcher`].
#[derive(Debug, Clone, Default)]
pub struct Legend {
    /// One or two sentences about what the mode shows.
    pub description: Text,
    /// A colour bar with its unit.
    pub scale: Option<ColorScale>,
    /// Colours, lines and markers with a meaning of their own (rivers, ice, boundaries, …).
    pub swatches: Vec<Swatch>,
    /// The meaning of the arrows' length, per kind of arrow.
    pub arrows: Vec<ArrowScale>,
}

/// A colour (or a line or marker in that colour) with its label.
#[derive(Debug, Clone, PartialEq)]
pub struct Swatch {
    pub color: [u8; 3],
    pub label: Text,
    pub shape: SwatchShape,
}

impl Swatch {
    /// A filled square of a colour.
    pub fn fill(color: [u8; 3], label: impl Into<Text>) -> Self {
        Self {
            color,
            label: label.into(),
            shape: SwatchShape::Fill,
        }
    }

    pub fn new(color: [u8; 3], shape: SwatchShape, label: impl Into<Text>) -> Self {
        Self {
            color,
            label: label.into(),
            shape,
        }
    }
}

/// How a [`Swatch`] is drawn; all but [`SwatchShape::Fill`] get a row of their own.
#[derive(Debug, Clone, PartialEq)]
pub enum SwatchShape {
    /// A filled square.
    Fill,
    /// A filled square on a row of its own, for categories with long labels.
    Entry,
    /// A line with a dark border, as boundary lines on the planet.
    Line,
    /// A line with teeth on one side, as subduction zones.
    Teeth,
    /// Dark dots on a background of this colour, as a stippled area.
    Dots,
    /// A row of small squares of these colours (the colour of the swatch is not used), as for
    /// categories whose colours have no meaning of their own.
    Palette(Vec<[u8; 3]>),
}

/// Arrows of known magnitudes, drawn in the legend at their size on the planet's front.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrowScale {
    /// What the arrows show, if the legend has several kinds.
    pub label: Option<Text>,
    /// Magnitudes in `unit` with the lengths and shaft widths of their arrows in planet radii.
    pub examples: Vec<(f64, f64, f64)>,
    pub unit: Text,
    /// The colour of the arrows.
    pub color: [f32; 4],
}

impl ArrowScale {
    /// The arrows of `magnitudes` in a style.
    pub fn new(style: &ArrowStyle, magnitudes: &[f64], unit: impl Into<Text>) -> Self {
        Self {
            label: None,
            examples: magnitudes
                .iter()
                .map(|&magnitude| (magnitude, style.length(magnitude), style.width(magnitude)))
                .collect(),
            unit: unit.into(),
            color: style.color,
        }
    }

    pub fn with_label(self, label: impl Into<Text>) -> Self {
        Self {
            label: Some(label.into()),
            ..self
        }
    }
}

/// A continuous colour scale: colours at values, interpolated linearly between them.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorScale {
    /// Values with their colours, sorted by value; at least two.
    pub stops: Vec<(f64, [u8; 3])>,
    /// What the values are, with their unit, e.g. "Elevation (km)".
    pub title: Text,
    /// Values labelled under the bar, with their labels.
    pub ticks: Vec<(f64, Text)>,
}

impl ColorScale {
    /// The colour of a value, clamped to the range of the scale; `None` for NaN.
    pub fn color(&self, value: f64) -> Option<[u8; 3]> {
        let (first, last) = (self.stops.first()?, self.stops.last()?);
        if value.is_nan() {
            return None;
        }
        if value <= first.0 {
            return Some(first.1);
        }
        if value >= last.0 {
            return Some(last.1);
        }
        let upper = self.stops.iter().position(|&(stop, _)| stop >= value)?;
        let (low, low_color) = self.stops[upper - 1];
        let (high, high_color) = self.stops[upper];
        let t = if high > low {
            (value - low) / (high - low)
        } else {
            1.0
        };
        Some(std::array::from_fn(|i| {
            let (a, b) = (f64::from(low_color[i]), f64::from(high_color[i]));
            (a + (b - a) * t).round() as u8
        }))
    }

    /// The range of the scale.
    pub fn range(&self) -> (f64, f64) {
        match (self.stops.first(), self.stops.last()) {
            (Some(first), Some(last)) => (first.0, last.0),
            _ => (0.0, 1.0),
        }
    }

    /// The colours of a per-cell value for [`RenderMode::colors`]: `missing` where the value is
    /// NaN.
    pub fn colors(&self, values: impl Iterator<Item = f32>, missing: [u8; 3]) -> Vec<[f32; 4]> {
        values
            .map(|value| rgba(self.color(f64::from(value)).unwrap_or(missing)))
            .collect()
    }
}

/// Draws the coastlines (between cells above and below sea level) as lines of `width` (planet
/// radii) in `color`, without border, lifted by `lift`: for modes that colour the
/// planet unlit, where the relief no longer shows land and sea.
pub fn coastlines(
    data: &ModeData,
    overlay: &mut OverlayBuilder,
    width: f64,
    color: [f32; 4],
    lift: f64,
) {
    if data.cells.len() != data.mesh.len() {
        return;
    }
    let pieces = category_boundaries(
        data.mesh.faces(),
        |cell| u32::from(data.cells[cell as usize].elevation >= 0.0),
        |cell| data.position(cell).as_dvec3(),
    );
    let style = LineStyle {
        width,
        border: 0.0,
        color,
        border_color: color,
        lift,
    };
    for piece in pieces {
        overlay.line(piece.from, piece.to, &style);
    }
}

/// An 8-bit sRGB colour as a vertex colour.
pub fn rgba([r, g, b]: [u8; 3]) -> [f32; 4] {
    [r, g, b, 255].map(|channel| f32::from(channel) / 255.0)
}

/// `vertices` with the colours of a mode, or `None` if the counts differ.
pub fn recolored(vertices: &[Vertex], colors: &[[f32; 4]]) -> Option<Vec<Vertex>> {
    (vertices.len() == colors.len()).then(|| {
        vertices
            .iter()
            .zip(colors)
            .map(|(vertex, &color)| Vertex { color, ..*vertex })
            .collect()
    })
}

/// Key of the remembered mode in the local storage of the web page.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Used by the web version.
pub const STORAGE_KEY: &str = "world-simulator.render-mode";

/// The mode the view starts with: the one in the URL (or on the command line), else the one
/// remembered from the last visit (if it is a valid name), else the default.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // Used by the web version.
pub fn initial(given: Option<RenderModeId>, stored: Option<&str>) -> RenderModeId {
    given
        .or_else(|| stored.and_then(RenderModeId::parse))
        .unwrap_or_default()
}

/// What to remember of a mode the user chose: its name, or nothing (forget) for the default.
pub fn to_store(mode: RenderModeId) -> Option<&'static str> {
    (mode != RenderModeId::default()).then(|| mode.name())
}

/// The name of the option on the command line and in the URL.
pub const OPTION: &str = "render-mode";

/// Command-line arguments (after the program name, as `--name=value`) with the render mode
/// replaced: `--render-mode=…` for any but the default, which is left out, so that the URL of
/// the web page names only a chosen mode.
pub fn with_render_mode(args: &[String], mode: RenderModeId) -> Vec<String> {
    let is_mode = |arg: &str| {
        arg.strip_prefix("--")
            .and_then(|arg| arg.strip_prefix(OPTION))
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('='))
    };
    let mut kept = Vec::with_capacity(args.len() + 1);
    let mut rest = args.iter();
    if let Some(program) = rest.next() {
        kept.push(program.clone());
    }
    while let Some(arg) = rest.next() {
        if arg == &format!("--{OPTION}") {
            // `--render-mode NAME`: the name goes too.
            rest.next();
        } else if !is_mode(arg) {
            kept.push(arg.clone());
        }
    }
    let mut args = kept;
    if let Some(name) = to_store(mode) {
        args.push(format!("--{OPTION}={name}"));
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_match_clap() {
        for &mode in RenderModeId::all() {
            let clap_name = mode.to_possible_value().unwrap().get_name().to_owned();
            assert_eq!(mode.name(), clap_name);
            assert_eq!(RenderModeId::parse(mode.name()), Some(mode));
            assert!(!mode.mode().label().is_empty());
            assert!(
                !mode
                    .mode()
                    .legend(&OptionValues::defaults(mode.mode().options()), None)
                    .description
                    .is_empty()
            );
        }
        assert_eq!(
            RenderModeId::parse("plate-motion"),
            Some(RenderModeId::PlateMotion)
        );
        assert_eq!(
            RenderModeId::parse(" topographic\n"),
            Some(RenderModeId::Topographic)
        );
        assert_eq!(RenderModeId::parse("nonsense"), None);
        assert_eq!(RenderModeId::default(), RenderModeId::Topographic);
    }

    #[test]
    fn the_url_wins_over_the_stored_mode_which_wins_over_the_default() {
        use RenderModeId::*;
        assert_eq!(initial(None, None), Topographic);
        assert_eq!(initial(None, Some("plate-motion")), PlateMotion);
        assert_eq!(
            initial(Some(Topographic), Some("plate-motion")),
            Topographic
        );
        assert_eq!(initial(Some(PlateMotion), None), PlateMotion);
        // A stored name that no mode has any more is ignored.
        assert_eq!(initial(None, Some("removed-mode")), Topographic);
        assert_eq!(initial(None, Some("")), Topographic);
    }

    #[test]
    fn the_default_is_forgotten_rather_than_stored() {
        assert_eq!(to_store(RenderModeId::Topographic), None);
        assert_eq!(to_store(RenderModeId::PlateMotion), Some("plate-motion"));
    }

    #[test]
    fn the_mode_replaces_the_one_in_the_arguments() {
        let strings = |args: &[&str]| args.iter().map(|&a| a.to_owned()).collect::<Vec<_>>();
        let args = strings(&["world-simulator", "--seed=3", "--render-mode=topographic"]);
        assert_eq!(
            with_render_mode(&args, RenderModeId::PlateMotion),
            strings(&["world-simulator", "--seed=3", "--render-mode=plate-motion"])
        );
        let args = strings(&["world-simulator", "--render-mode=plate-motion", "--seed=3"]);
        assert_eq!(
            with_render_mode(&args, RenderModeId::Topographic),
            strings(&["world-simulator", "--seed=3"])
        );
        // The spelling with a separate value.
        let args = strings(&[
            "world-simulator",
            "--render-mode",
            "plate-motion",
            "--seed=3",
        ]);
        assert_eq!(
            with_render_mode(&args, RenderModeId::Topographic),
            strings(&["world-simulator", "--seed=3"])
        );
        // Other options that start with the same letters stay.
        let args = strings(&["world-simulator", "--render-mode-x=1"]);
        assert_eq!(with_render_mode(&args, RenderModeId::Topographic), args);
        // The arguments with the mode parse.
        use clap::Parser;
        let cli = crate::Cli::try_parse_from(with_render_mode(
            &strings(&["world-simulator"]),
            RenderModeId::PlateMotion,
        ))
        .unwrap();
        assert_eq!(cli.render_mode, RenderModeId::PlateMotion);
    }

    #[test]
    fn color_scales_interpolate_and_clamp() {
        let scale = ColorScale {
            stops: vec![
                (0.0, [0, 0, 0]),
                (10.0, [100, 200, 0]),
                (20.0, [100, 0, 250]),
            ],
            title: "Height (m)".into(),
            ticks: vec![(0.0, "0".into()), (20.0, "20".into())],
        };
        assert_eq!(scale.color(-5.0), Some([0, 0, 0]));
        assert_eq!(scale.color(5.0), Some([50, 100, 0]));
        assert_eq!(scale.color(10.0), Some([100, 200, 0]));
        assert_eq!(scale.color(15.0), Some([100, 100, 125]));
        assert_eq!(scale.color(99.0), Some([100, 0, 250]));
        assert_eq!(scale.color(f64::NAN), None);
        assert_eq!(scale.range(), (0.0, 20.0));
        let colors = scale.colors([5.0, f32::NAN].into_iter(), [1, 2, 3]);
        assert_eq!(colors[0], rgba([50, 100, 0]));
        assert_eq!(colors[1], rgba([1, 2, 3]));
    }

    /// A scalar mode as later modes will be: colours from a [`CellInfo`] attribute.
    struct SedimentTest;

    impl RenderMode for SedimentTest {
        fn label(&self) -> &'static str {
            "Sediment (test)"
        }

        fn colors(&self, data: &ModeData) -> Option<Vec<[f32; 4]>> {
            let scale = ColorScale {
                stops: vec![(0.0, [0, 0, 0]), (1000.0, [255, 255, 255])],
                title: "Sediment (m)".into(),
                ticks: vec![],
            };
            Some(scale.colors(data.cells.iter().map(|cell| cell.sediment), [255, 0, 0]))
        }

        fn shading(&self) -> Shading {
            Shading::Unlit
        }

        fn legend(&self, _options: &OptionValues, _data: Option<&ModeData>) -> Legend {
            Legend {
                description: "Sediment thickness.".into(),
                ..Legend::default()
            }
        }
    }

    #[test]
    fn a_scalar_mode_recolours_the_vertices() {
        use clap::Parser;
        let cli = crate::Cli::parse_from(["world-simulator", "--subdivisions", "2"]);
        let planet = crate::build_planet(&cli).unwrap();
        let cells = super::super::cell_infos(&planet);
        let mesh = planet.tectonics().mesh();
        let vertices = super::super::surface_vertices(mesh, &cells, 40.0, 1e11);
        let fields = Fields::default();
        let plate_colors = PlateColors::default();
        let data = ModeData {
            mesh,
            vertices: &vertices,
            cells: &cells,
            plates: &[],
            plate_colors: &plate_colors,
            fields: &fields,
            arrow_cells: &[],
            arrow_spacing: 0.1,
            options: &OptionValues::default(),
            abiotic: false,
        };
        let colors = SedimentTest.colors(&data).unwrap();
        let new_vertices = recolored(&vertices, &colors).unwrap();
        for ((vertex, new), cell) in vertices.iter().zip(&new_vertices).zip(&cells) {
            assert_eq!(vertex.position, new.position);
            assert_eq!(vertex.normal, new.normal);
            let grey = (cell.sediment / 1000.0).clamp(0.0, 1.0);
            assert!(
                (new.color[0] - grey).abs() < 0.01,
                "{} {grey}",
                new.color[0]
            );
        }
        assert!(recolored(&vertices, &colors[1..]).is_none());
        assert_eq!(SedimentTest.shading(), Shading::Unlit);
        assert_eq!(RenderModeId::Topographic.mode().shading(), Shading::Lit);
        // The topographic mode keeps the colours and draws nothing over them; the plate motion
        // mode draws no arrows without its data.
        assert!(RenderModeId::Topographic.mode().colors(&data).is_none());
        let mut overlay = OverlayBuilder::default();
        RenderModeId::Topographic
            .mode()
            .overlay(&data, &mut overlay);
        RenderModeId::PlateMotion
            .mode()
            .overlay(&data, &mut overlay);
        assert!(overlay.is_empty());
    }
}
