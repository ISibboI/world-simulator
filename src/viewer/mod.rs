//! Interactive 3D view of the planet.
//!
//! The window is an [`eframe`] (egui) application, which runs on Windows, macOS and Linux, and
//! lets later user interface elements be drawn on top of the 3D view. The planet itself is
//! drawn with wgpu through an egui paint callback (see [`renderer`]).
//!
//! The simulation runs on a separate thread (in a Web Worker on the web, see `crate::web`) and
//! sends every new state to the view through a channel: the attributes of every cell
//! ([`CellInfo`]) and the rotation of every plate; the view builds the surface mesh from them.
//! Every state is also recorded in the frame history ([`history`]), so that the time bar
//! ([`timeline`]) can show any past one; it also pauses and steps the simulation. The parameter panel ([`parameters`]) restarts it with other options; the
//! cell widget ([`cell_info`]) shows the cell under the pointer (or the tapped one), whose
//! outline is drawn on the planet ([`picking`]). A banner ([`banner`]) shows what the
//! simulation is doing until its first update (starting, generating the planet), or that it
//! failed. The simulation reports how long each step
//! took, and a hint proposes other subdivisions if steps are too slow or very quick ([`pace`]).
//! The render mode ([`modes`]) decides what the planet shows: the topographic colours by
//! default, or others chosen in the mode switcher, with a legend. Modes that need more than the
//! cell attributes request extra fields ([`fields`]) from the simulation, which sends them with
//! its updates; vector modes draw arrows ([`arrows`]).

// The view only shows the simulation and never changes it, so it may use the platform's math
// functions directly instead of `crate::math` (see `clippy.toml`).
#![allow(clippy::disallowed_methods)]

mod arrows;
mod atmosphere;
mod banner;
mod cell_info;
mod fields;
mod global;
mod history;
mod hydrology;
mod lighting;
mod modes;
mod overlay;
mod pace;
mod parameters;
mod picking;
mod plate_colors;
mod renderer;
mod settings;
mod sky;
mod timeline;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use eframe::egui;
use glam::{DVec3, Vec2, Vec3};

use crate::Cli;
use crate::planet::Planet;
use crate::render;
use crate::sphere::SphereMesh;
use arrows::OverlayVertex;
pub use cell_info::{CellInfo, cell_infos};
use cell_info::{CellInfoPanel, ShownCell};
#[cfg(target_arch = "wasm32")]
pub use fields::Layer;
pub use fields::{Fields, Layers, PlateInfo, plate_infos};
pub use global::GlobalStatus;
#[cfg(target_arch = "wasm32")]
pub use history::store::MemoryStore;
pub use history::{FrameEncoder, FrameStore, History};
use lighting::{Light, Lighting, SunView};
pub use modes::RenderModeId;
use modes::switcher::{ModeSwitcher, SwitcherView};
use modes::{ModeData, Shading, recolored};
#[cfg(target_arch = "wasm32")]
pub use modes::{
    STORAGE_KEY as RENDER_MODE_STORAGE_KEY, initial as initial_render_mode, to_store,
    with_render_mode,
};
use overlay::OverlayBuilder;
use pace::{HintAction, Pace};
pub use parameters::with_random_seed;
use parameters::{ParameterPanel, SimulationStatus};
#[cfg(target_arch = "wasm32")]
pub use parameters::{has_option, with_default};
use plate_colors::PlateColors;
use renderer::Vertex;
use renderer::{Detail, LineSegment, Overlay, PlanetCallback, Surface, Uniforms};
pub use settings::Settings;

/// Bits of the depth buffer requested from eframe; must match [`renderer::DEPTH_FORMAT`].
const DEPTH_BUFFER_BITS: u8 = 32;
/// Multisample anti-aliasing samples (desktop only; the web painter does not multisample).
#[cfg(not(target_arch = "wasm32"))]
const MSAA_SAMPLES: u16 = 4;
/// Field of view of the camera in radians: vertical, or horizontal in a portrait view.
const FIELD_OF_VIEW: f32 = 0.6;
/// Distance of the camera from the planet centre in planet radii.
const CAMERA_DISTANCE: f32 = 4.5;
/// Camera rotation in radians per point of mouse movement.
const ROTATION_SPEED: f32 = 0.008;
/// Width of the outline of the shown cell in points: a light line with a dark border, visible
/// on any terrain.
const OUTLINE_WIDTH: f32 = 1.5;
const OUTLINE_BORDER_WIDTH: f32 = 3.5;
/// The outline is moved towards the camera by this fraction of the cell spacing, so that it is
/// not hidden by the surface it lies on (also where its width reaches over a slope).
const OUTLINE_LIFT: f64 = 0.5;
/// Arrows of vector modes stand on the cells nearest to the vertices of an icosphere with this
/// many subdivisions: 642 arrows about 8° (900 km) apart, readable at the default zoom.
const ARROW_SUBDIVISIONS: u32 = 3;

/// A new state of the planet surface to display.
pub struct SurfaceUpdate {
    /// Simulated time in Myr.
    pub time: f64,
    /// The attributes of every cell, in the order of the mesh vertices.
    pub cells: Arc<Vec<CellInfo>>,
    /// Every plate, by index ([`plate_infos`]).
    pub plates: Arc<Vec<PlateInfo>>,
    /// The state as a frame of the history, encoded by the simulation with its
    /// [`FrameEncoder`]: `None` for a repeated state (sent again with other fields).
    pub frame: Option<Vec<u8>>,
    /// The mesh: sent with the first update of a simulation, `None` after.
    pub mesh: Option<Arc<MeshUpdate>>,
    /// Seconds since the previous update, i.e. the time of the step: `None` for the initial
    /// state, for a repeated state (sent again with other fields), or if it is not
    /// representative (the web page was hidden meanwhile).
    pub step_seconds: Option<f64>,
    /// The extra fields the view requested (see [`SimulationControl::request_layers`]).
    pub fields: Fields,
    /// The global state of the run (the ramp-up phase), shown with its time.
    pub global: GlobalStatus,
}

/// The icosphere of a simulation: the directions of its vertices (one per cell, unit length)
/// and its triangles.
pub struct MeshUpdate {
    pub directions: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
}

impl MeshUpdate {
    pub fn of(planet: &Planet) -> Self {
        let mesh = planet.tectonics().mesh();
        Self {
            directions: (0..mesh.len() as u32)
                .map(|cell| mesh.vertex(cell).as_vec3().to_array())
                .collect(),
            faces: mesh.faces().to_vec(),
        }
    }
}

/// A message from the simulation to the view.
pub enum SimulationEvent {
    /// The simulation is generating its initial planet (it sends no surface before).
    Generating,
    Surface(SurfaceUpdate),
    /// The simulation failed with this message.
    Failed(String),
    /// The simulation stopped for a pause, inside a step or between steps (see
    /// [`crate::pausing`]).
    Halted(crate::pausing::Halt),
}

/// A simulation to start: its options and the command-line arguments they came from (starting
/// with the program name; the web version passes them to the worker as its URL query).
pub struct Launch {
    pub cli: Cli,
    pub args: Vec<String>,
}

/// Adjustments of the user interface to the device, see [`UiOptions::apply`]. The default
/// keeps egui's style (the desktop).
#[derive(Debug, Clone, Copy, Default)]
pub struct UiOptions {
    /// Size of body text in points; the other text styles are scaled with it. On the web it is
    /// the browser's root font size in CSS pixels, which follows the user's font size setting.
    pub body_text_size: Option<f32>,
    /// Whether the main pointer is coarse (a touch screen): larger targets and spacing.
    pub touch: bool,
    /// The default of the subdivisions in the parameter panel if it differs from the
    /// command-line default: on the web, the guess for the device (see `crate::device`).
    pub default_subdivisions: Option<u32>,
    /// Most subdivisions the step-time hint proposes, if not [`pace::MAX_RECOMMENDED`].
    pub max_recommended_subdivisions: Option<u32>,
}

/// Smallest size of interactive elements on touch screens in points (CSS pixels on the web),
/// as recommended by Apple (44 pt) and Google (48 dp).
const TOUCH_TARGET: f32 = 44.0;

impl UiOptions {
    fn apply(&self, ctx: &egui::Context) {
        let options = *self;
        ctx.all_styles_mut(move |style| {
            if let Some(body_text_size) = options.body_text_size
                && let Some(body) = style.text_styles.get(&egui::TextStyle::Body)
                && body.size > 0.0
            {
                let scale = body_text_size / body.size;
                for font in style.text_styles.values_mut() {
                    font.size *= scale;
                }
            }
            if options.touch {
                let spacing = &mut style.spacing;
                spacing.interact_size = egui::vec2(TOUCH_TARGET, TOUCH_TARGET);
                spacing.button_padding = egui::vec2(12.0, 10.0);
                spacing.item_spacing = egui::vec2(10.0, 10.0);
                spacing.icon_width = 28.0;
                spacing.icon_width_inner = 16.0;
                spacing.icon_spacing = 8.0;
                spacing.combo_height = 400.0;
            }
        });
    }
}

/// Builds the surface mesh vertices of a state from the attributes of its cells.
///
/// The drawn surface ([`CellInfo::surface_height`]: the ground of land, also of dry closed
/// depressions below sea level, the water level of lakes) is raised above the sphere by its
/// height times `exaggeration`; the ocean surface is at radius 1. The colours are the
/// topographic ones (`render::surface_color`).
fn surface_vertices(
    mesh: &SphereMesh,
    cells: &[CellInfo],
    exaggeration: f64,
    river_threshold: f64,
) -> Vec<Vertex> {
    let radius = crate::EARTH_RADIUS;
    let positions: Vec<DVec3> = cells
        .iter()
        .enumerate()
        .map(|(cell, info)| {
            let height = info.surface_height() * exaggeration / radius;
            mesh.vertex(cell as u32) * (1.0 + height)
        })
        .collect();

    let mut normals = vec![DVec3::ZERO; positions.len()];
    for &[a, b, c] in mesh.faces() {
        let (pa, pb, pc) = (
            positions[a as usize],
            positions[b as usize],
            positions[c as usize],
        );
        // Area-weighted face normal.
        let normal = (pb - pa).cross(pc - pa);
        for vertex in [a, b, c] {
            normals[vertex as usize] += normal;
        }
    }

    cells
        .iter()
        .zip(positions.iter().zip(&normals))
        .map(|(info, (position, normal))| {
            let image::Rgb(color) = render::surface_color(
                f64::from(info.elevation),
                info.water(),
                f64::from(info.ice),
                f64::from(info.discharge),
                river_threshold,
            );
            Vertex {
                position: position.as_vec3().to_array(),
                normal: normal.normalize_or_zero().as_vec3().to_array(),
                color: [
                    f32::from(color[0]) / 255.0,
                    f32::from(color[1]) / 255.0,
                    f32::from(color[2]) / 255.0,
                    1.0,
                ],
            }
        })
        .collect()
}

/// Starts a simulation (on another thread, or in a Web Worker on the web) that sends its events
/// through the sender and calls the callback to wake up the view after each, and returns its
/// control. Called once the view exists, and again on every restart from the parameter panel,
/// with a new channel: a previous simulation should stop when sending to its channel fails or
/// its control is dropped. The simulation starts with the layers of `launch.cli.render_mode`.
pub type StartSimulation = Box<
    dyn FnMut(Launch, Sender<SimulationEvent>, Box<dyn Fn() + Send>) -> Box<dyn SimulationControl>,
>;

/// The view's line to a running simulation.
pub trait SimulationControl {
    /// Asks the simulation to send these extra fields with its updates from now on. A
    /// simulation sends its current state again with them if they include new ones: at once if
    /// it is between steps (also once it has finished), else with the step in progress (a
    /// paused one only once the step is complete). Until then the view builds them from the shown
    /// state, as for past frames.
    fn request_layers(&mut self, layers: Layers);

    /// Pauses the simulation at the next point where its state is consistent, inside a step or
    /// between steps (it then reports [`SimulationEvent::Halted`]), or resumes it. The step after
    /// a pause is sent without its time (`SurfaceUpdate::step_seconds`), which includes the
    /// pause. `request` numbers the requests of the view (pause, resume and step), so that it can
    /// tell which one a [`SimulationEvent::Halted`] answers.
    fn set_paused(&mut self, paused: bool, request: u64);

    /// Takes one step while paused, and stays paused: the next step, or the rest of the current
    /// one if the simulation stopped (or is stopping) inside a step.
    fn step(&mut self, request: u64);
}

/// How long the view waits for the simulation to stop after a pause before it says that it is
/// pausing.
const PAUSING_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

/// Opens the store of the frame history of a new run; the callback wakes the view when a read
/// completes. Called on every start of a simulation.
pub type OpenFrameStore = Box<dyn FnMut(Box<dyn Fn()>) -> Box<dyn FrameStore>>;

/// Bytes the desktop keeps of the frame history of a run (on disk); at the measured
/// ~0.33 MB per frame at 6 subdivisions, about 12,000 frames.
#[cfg(not(target_arch = "wasm32"))]
const DESKTOP_HISTORY_CAPACITY: u64 = 4 << 30;

/// The store of the desktop: a temporary file, or memory if none can be created.
#[cfg(not(target_arch = "wasm32"))]
fn open_file_store() -> OpenFrameStore {
    Box::new(
        |_wake| match history::store::FileStore::new(DESKTOP_HISTORY_CAPACITY) {
            Ok(store) => Box::new(store),
            Err(error) => {
                tracing::warn!(
                    "No temporary file for the frame history, keeping it in memory: {error}"
                );
                Box::new(history::store::MemoryStore::new(1 << 30))
            }
        },
    )
}

/// Called when the user chose another render mode (the web page puts it into its URL and
/// remembers it).
pub type RememberRenderMode = Box<dyn FnMut(RenderModeId)>;

/// Creates the viewer app; shared by the desktop window and the web page.
#[allow(clippy::too_many_arguments)]
fn app_creator(
    initial: Launch,
    msaa_samples: u16,
    ui_options: UiOptions,
    start_simulation: StartSimulation,
    open_store: OpenFrameStore,
    remember_render_mode: Option<RememberRenderMode>,
    settings: Box<dyn Settings>,
    on_first_frame: Option<Box<dyn FnOnce()>>,
) -> eframe::AppCreator<'static> {
    Box::new(move |creation_context| {
        let render_state = creation_context
            .wgpu_render_state
            .as_ref()
            .ok_or("the wgpu renderer is not available")?;
        renderer::initialize(render_state, u32::from(msaa_samples));
        ui_options.apply(&creation_context.egui_ctx);
        Ok(Box::new(Viewer::new(
            &creation_context.egui_ctx,
            initial,
            ui_options,
            start_simulation,
            open_store,
            remember_render_mode,
            settings,
            on_first_frame,
        )))
    })
}

/// The window icon: the planet of the favicon (issue #124, `web/icons/`).
#[cfg(not(target_arch = "wasm32"))]
fn window_icon() -> egui::IconData {
    let image = image::load_from_memory_with_format(
        include_bytes!("../../web/icons/icon-256.png"),
        image::ImageFormat::Png,
    )
    .expect("the embedded window icon is a valid PNG")
    .into_rgba8();
    egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

/// Opens the 3D view and blocks until the window is closed.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(initial: Launch, start_simulation: StartSimulation) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("World Simulator")
            .with_inner_size([1200.0, 900.0])
            .with_icon(window_icon()),
        depth_buffer: DEPTH_BUFFER_BITS,
        multisampling: MSAA_SAMPLES,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "World Simulator",
        options,
        app_creator(
            initial,
            MSAA_SAMPLES,
            UiOptions::default(),
            start_simulation,
            open_file_store(),
            None,
            Box::new(settings::FileSettings::open()),
            None,
        ),
    )
    .map_err(|error| anyhow::anyhow!("failed to run the 3D view: {error}"))?;
    tracing::info!("3D view closed");
    Ok(())
}

/// Starts the 3D view in `canvas` of the web page and calls `on_first_frame` at the end of its
/// first frame, which is shown together with the page changes made then (the page hides its
/// loading banner, which the view's banner replaces in the same place). eframe's web painter
/// does not multisample.
#[cfg(target_arch = "wasm32")]
#[allow(clippy::too_many_arguments)]
pub async fn run_web(
    canvas: web_sys::HtmlCanvasElement,
    initial: Launch,
    ui_options: UiOptions,
    start_simulation: StartSimulation,
    open_store: OpenFrameStore,
    remember_render_mode: RememberRenderMode,
    settings: Box<dyn Settings>,
    on_first_frame: Box<dyn FnOnce()>,
) -> anyhow::Result<()> {
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.device_descriptor = std::sync::Arc::new(web_device_descriptor);
    }
    let options = eframe::WebOptions {
        depth_buffer: DEPTH_BUFFER_BITS,
        wgpu_options,
        ..Default::default()
    };
    eframe::WebRunner::new()
        .start(
            canvas,
            options,
            app_creator(
                initial,
                1,
                ui_options,
                start_simulation,
                open_store,
                Some(remember_render_mode),
                settings,
                Some(on_first_frame),
            ),
        )
        .await
        .map_err(|error| anyhow::anyhow!("failed to start the 3D view: {error:?}"))
}

/// The device of the web view: egui's default (WebGL 2 limits on WebGL), but textures only as
/// large as the adapter allows. egui asks for 8192 pixels (for depth buffers of 4K+ screens),
/// which some phones' and emulators' WebGL do not have (4096), and then the view did not start.
#[cfg(target_arch = "wasm32")]
fn web_device_descriptor(
    adapter: &eframe::wgpu::Adapter,
) -> eframe::wgpu::DeviceDescriptor<'static> {
    let base = if adapter.get_info().backend == eframe::wgpu::Backend::Gl {
        eframe::wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        eframe::wgpu::Limits::default()
    };
    let available = adapter.limits().max_texture_dimension_2d;
    eframe::wgpu::DeviceDescriptor {
        label: Some("egui wgpu device"),
        required_limits: eframe::wgpu::Limits {
            max_texture_dimension_2d: available.min(8192),
            ..base
        },
        ..Default::default()
    }
}

struct Viewer {
    events: Receiver<SimulationEvent>,
    start_simulation: StartSimulation,
    open_store: OpenFrameStore,
    /// The frame history of the running simulation.
    history: Option<History>,
    /// The newest state of the running simulation, to return to from a past frame.
    latest: Option<Latest>,
    /// The recorded frame shown (or being read), or `None` while the view follows the newest
    /// state.
    viewing: Option<usize>,
    /// The recorded frame on screen (`viewing` once its read completed), and the pace of a
    /// playing view, through the recorded frames or live (see [`timeline::Walk`]).
    shown_frame: Option<usize>,
    walk: timeline::Walk,
    /// The view is live and has not shown the newest state yet (it waits for its pace).
    live_pending: bool,
    /// Why the past frame could not be shown.
    frame_error: Option<String>,
    /// Whether the user paused the simulation, and asked for a step that has not arrived.
    paused: bool,
    step_pending: bool,
    /// The number of the latest request to the simulation (pause, resume, step), when the latest
    /// pause or step was asked for, and where the simulation stopped since (inside a step or
    /// not), if it answered the latest request (see [`SimulationControl::set_paused`]).
    requests: u64,
    request_time: Option<web_time::Instant>,
    halted: Option<bool>,
    /// Options of the running simulation for building the surface: relief exaggeration, river
    /// threshold, and the time step (Myr).
    exaggeration: f64,
    river_threshold: f64,
    /// Whether the running simulation has no life (`--biosphere abiotic`).
    abiotic: bool,
    dt: f64,
    /// The epoch of the running simulation (`--epoch`), for the sun.
    epoch: f64,
    /// Simulated time (Myr) of the displayed state, for the age of the sun.
    shown_time: f64,
    /// The control of the running simulation.
    control: Option<Box<dyn SimulationControl>>,
    remember_render_mode: Option<RememberRenderMode>,
    /// The active render mode, its switcher, and the layers requested for it.
    mode: RenderModeId,
    switcher: ModeSwitcher,
    requested: Layers,
    /// The extra fields of the latest surface.
    fields: Fields,
    /// What is drawn: the latest surface in the colours of the mode, and the mode's overlay;
    /// rebuilt when the surface or the mode changes ([`Viewer::refresh_display`]).
    display: Option<Surface>,
    overlay: Option<Overlay>,
    display_outdated: bool,
    /// Ids of the displayed vertices and overlays, for the renderer.
    displays: u64,
    /// The cells arrows stand on, for [`Picking::mesh_id`], and their spacing.
    arrow_cells: Option<(u64, Vec<u32>, f64)>,
    /// Points per planet radius on the planet's front in the last frame.
    points_per_radius: f32,
    /// The legend of the active mode for the displayed state.
    legend: modes::Legend,
    parameters: ParameterPanel,
    /// The colour of every plate of the run, for the plate mode.
    plate_colors: PlateColors,
    cell_panel: CellInfoPanel,
    /// The time bar along the bottom edge, collapsed or expanded.
    time_bar: timeline::TimeBar,
    /// Where choices of the user are remembered across restarts.
    settings: Box<dyn Settings>,
    pace: Pace,
    /// Whether the main pointer is coarse (touch): cells are tapped rather than pointed at.
    touch: bool,
    /// The mesh of [`Viewer::surface`] for picking, with its cells' attributes.
    picking: Option<Picking>,
    /// The cell selected by a click or tap, shown while no other cell is under the pointer.
    selected: Option<u32>,
    /// The cell under the pointer in the last frame, where the next pick starts its walk.
    hovered: Option<u32>,
    /// Rotation of the camera around the planet's axis in radians.
    yaw: f32,
    /// Elevation angle of the camera above the equator in radians.
    pitch: f32,
    /// The exposure of the sky's view as it adapts to the scene, and when it was last updated
    /// (see [`sky::Adaptation`]).
    adaptation: sky::Adaptation,
    adapted_at: Option<web_time::Instant>,
    /// Simulated time of the newest state of the running simulation.
    time: Option<f64>,
    /// The global state of the newest state of the running simulation (see [`GlobalStatus`]).
    global: Option<GlobalStatus>,
    /// The global state of the displayed state (live or a past frame), for the panel (#209).
    shown_global: Option<GlobalStatus>,
    /// Simulated duration of the running simulation in Myr.
    duration: f64,
    /// Why the running simulation failed, if it did.
    failure: Option<String>,
    /// The number of simulations started: more than one after a restart.
    runs: u32,
    /// Whether the running simulation reported that it is generating its planet.
    generating: bool,
    /// Called at the end of the first frame (the web page hides its loading banner then).
    on_first_frame: Option<Box<dyn FnOnce()>>,
    /// The displayed surface; the previous simulation's until the new one sends its first.
    surface: Option<Surface>,
    /// The number of meshes and surface states received, for [`Surface`]'s ids.
    meshes: u64,
    surfaces: u64,
    /// Intervals between frames, logged at debug level (see [`FrameTimes`]).
    frame_times: FrameTimes,
}

/// Collects the intervals between frames and logs their median and 90th percentile every
/// [`FrameTimes::PERIOD`] seconds at debug level, while the view redraws (e.g. while it is
/// dragged): the frame rate of the realistic mode's quality levels on slow devices.
#[derive(Default)]
struct FrameTimes {
    last: Option<web_time::Instant>,
    since: Option<web_time::Instant>,
    intervals: Vec<f32>,
}

impl FrameTimes {
    const PERIOD: f32 = 2.0;
    /// Longer pauses are idle time, not frames.
    const IDLE: f32 = 0.5;

    fn frame(&mut self, mode: RenderModeId) {
        let now = web_time::Instant::now();
        if let Some(last) = self.last.replace(now) {
            let seconds = (now - last).as_secs_f32();
            if seconds < Self::IDLE {
                self.intervals.push(seconds);
            }
        }
        let since = *self.since.get_or_insert(now);
        if (now - since).as_secs_f32() >= Self::PERIOD {
            self.since = Some(now);
            let mut intervals = std::mem::take(&mut self.intervals);
            if intervals.len() >= 5 {
                intervals.sort_by(f32::total_cmp);
                let at = |share: f32| intervals[((intervals.len() - 1) as f32 * share) as usize];
                tracing::debug!(
                    mode = mode.name(),
                    frames = intervals.len(),
                    median_ms = at(0.5) * 1000.0,
                    p90_ms = at(0.9) * 1000.0,
                    "Frame times"
                );
            }
        }
    }
}

/// The newest state of the running simulation.
struct Latest {
    time: f64,
    global: GlobalStatus,
    cells: Arc<Vec<CellInfo>>,
    plates: Arc<Vec<PlateInfo>>,
    fields: Fields,
}

/// The mesh of the displayed surface, and what picking needs to know about it.
struct Picking {
    /// [`Surface::mesh_id`] of the mesh.
    mesh_id: u64,
    /// The mesh with the directions of the cells.
    mesh: SphereMesh,
    faces: Arc<Vec<[u32; 3]>>,
    /// The attributes of the cells of the displayed state, and its plates.
    cells: Arc<Vec<CellInfo>>,
    plates: Arc<Vec<PlateInfo>>,
    /// The drainage basins of the displayed state, for the cell widget.
    drainage: hydrology::Drainage,
    /// The largest displayed radius of a vertex.
    max_radius: f64,
}

impl Viewer {
    #[allow(clippy::too_many_arguments)]
    fn new(
        ctx: &egui::Context,
        initial: Launch,
        ui_options: UiOptions,
        start_simulation: StartSimulation,
        open_store: OpenFrameStore,
        remember_render_mode: Option<RememberRenderMode>,
        settings: Box<dyn Settings>,
        on_first_frame: Option<Box<dyn FnOnce()>>,
    ) -> Self {
        let parameters = ParameterPanel::new(&initial.args, ui_options.default_subdivisions);
        let pace = Pace::new(
            &initial.cli,
            ui_options
                .max_recommended_subdivisions
                .unwrap_or(pace::MAX_RECOMMENDED),
        );
        let (_, events) = std::sync::mpsc::channel();
        let mut viewer = Self {
            events,
            start_simulation,
            open_store,
            history: None,
            latest: None,
            viewing: None,
            shown_frame: None,
            walk: timeline::Walk::default(),
            live_pending: false,
            frame_error: None,
            paused: false,
            step_pending: false,
            requests: 0,
            request_time: None,
            halted: None,
            exaggeration: initial.cli.exaggeration,
            abiotic: initial.cli.biosphere == crate::presets::Biosphere::Abiotic,
            river_threshold: initial.cli.river_threshold,
            dt: initial.cli.dt,
            epoch: initial.cli.epoch,
            shown_time: 0.0,
            control: None,
            remember_render_mode,
            mode: initial.cli.render_mode,
            switcher: ModeSwitcher::default(),
            requested: Layers::NONE,
            fields: Fields::default(),
            display: None,
            overlay: None,
            display_outdated: false,
            displays: 0,
            arrow_cells: None,
            points_per_radius: 100.0,
            legend: {
                let mode = initial.cli.render_mode.mode();
                mode.legend(&modes::OptionValues::defaults(mode.options()), None)
            },
            parameters,
            plate_colors: PlateColors::default(),
            cell_panel: CellInfoPanel::default(),
            time_bar: timeline::TimeBar::new(settings::get_bool(
                settings.as_ref(),
                timeline::STORAGE_KEY,
            )),
            settings,
            pace,
            touch: ui_options.touch,
            picking: None,
            selected: None,
            hovered: None,
            yaw: 0.0,
            pitch: 0.4,
            adaptation: sky::Adaptation::default(),
            adapted_at: None,
            time: None,
            global: None,
            shown_global: None,
            duration: initial.cli.run_duration(),
            failure: None,
            runs: 0,
            generating: false,
            on_first_frame,
            surface: None,
            meshes: 0,
            surfaces: 0,
            frame_times: FrameTimes::default(),
        };
        viewer.start(ctx, initial);
        viewer
    }

    /// Starts a simulation with a new channel, which also stops the previous one (see
    /// [`StartSimulation`]). The view keeps showing the previous planet until the new one
    /// arrives.
    fn start(&mut self, ctx: &egui::Context, mut launch: Launch) {
        // Dropping the control first lets a finished simulation end (see `StartSimulation`).
        self.control = None;
        // The simulation starts with the fields of the active mode, and the web page puts the
        // mode into its URL with the other options.
        launch.args = modes::with_render_mode(&launch.args, self.mode);
        launch.cli.render_mode = self.mode;
        self.requested = self.mode.mode().layers();
        let (sender, receiver) = std::sync::mpsc::channel();
        self.events = receiver;
        self.time = None;
        self.global = None;
        self.shown_global = None;
        self.duration = launch.cli.run_duration();
        self.exaggeration = launch.cli.exaggeration;
        self.abiotic = launch.cli.biosphere == crate::presets::Biosphere::Abiotic;
        self.river_threshold = launch.cli.river_threshold;
        self.dt = launch.cli.dt;
        self.epoch = launch.cli.epoch;
        self.shown_time = 0.0;
        // A new history (the old one is deleted first); the view shows the previous planet,
        // without time controls, until the new simulation sends its first state.
        self.history = None;
        let wake = ctx.clone();
        self.history = Some(History::new((self.open_store)(Box::new(move || {
            wake.request_repaint()
        }))));
        self.latest = None;
        self.viewing = None;
        self.shown_frame = None;
        // The first state of the new simulation shows at once.
        let rate = self.walk.rate;
        self.walk = timeline::Walk::default();
        self.walk.rate = rate;
        self.live_pending = false;
        self.frame_error = None;
        self.paused = false;
        self.step_pending = false;
        self.request_time = None;
        self.halted = None;
        self.failure = None;
        self.runs += 1;
        self.generating = false;
        // Shows the banner at once, not only with the next input or event.
        ctx.request_repaint();
        self.pace.start(&launch.cli);
        let ctx = ctx.clone();
        self.control = Some((self.start_simulation)(
            launch,
            sender,
            Box::new(move || ctx.request_repaint()),
        ));
    }

    /// Switches to another render mode: requests its fields and remembers it.
    fn set_mode(&mut self, mode: RenderModeId) {
        tracing::info!(mode = mode.name(), "Render mode");
        let exaggeration = self.shown_exaggeration();
        self.mode = mode;
        self.display_outdated = true;
        let layers = mode.mode().layers();
        // Fields the shown state lacks are rebuilt from its cells and plates, as for past frames,
        // until the simulation sends them (between steps, see `SimulationControl::request_layers`):
        // a simulation paused inside a step sends none.
        let missing = self.fields.layers().0 & layers.0 != layers.0;
        // The realistic mode shows the relief true to scale: rebuild the surface.
        if (self.shown_exaggeration() != exaggeration || missing)
            && let Some(picking) = &self.picking
            && !picking.cells.is_empty()
        {
            let (cells, plates) = (picking.cells.clone(), picking.plates.clone());
            let fields = if missing {
                Fields::from_frame(&picking.mesh, &cells, &plates)
            } else {
                self.fields.clone()
            };
            self.show(self.shown_time, cells, plates, fields);
        }
        if layers != self.requested {
            self.requested = layers;
            if let Some(control) = &mut self.control {
                control.request_layers(layers);
            }
        }
        if let Some(remember) = &mut self.remember_render_mode {
            remember(mode);
        }
    }

    /// Rebuilds what is drawn for the latest surface and the active mode, if either changed.
    fn refresh_display(&mut self) {
        if !std::mem::take(&mut self.display_outdated) {
            return;
        }
        let (Some(surface), Some(picking)) = (&self.surface, &self.picking) else {
            // A fresh id, so that the renderer uploads these vertices.
            self.displays += 1;
            self.display = self.surface.clone().map(|surface| Surface {
                vertices_id: self.displays,
                ..surface
            });
            self.overlay = None;
            self.legend = self
                .mode
                .mode()
                .legend(self.switcher.options(self.mode), None);
            return;
        };
        let start = web_time::Instant::now();
        if self
            .arrow_cells
            .as_ref()
            .is_none_or(|(mesh_id, ..)| *mesh_id != picking.mesh_id)
        {
            let (cells, spacing) = arrows::sample_cells(&picking.mesh, ARROW_SUBDIVISIONS);
            self.arrow_cells = Some((picking.mesh_id, cells, spacing));
        }
        let (arrow_cells, arrow_spacing) = match &self.arrow_cells {
            Some((_, cells, spacing)) => (cells.as_slice(), *spacing),
            None => (&[][..], 0.1),
        };
        let data = ModeData {
            mesh: &picking.mesh,
            vertices: &surface.vertices,
            cells: &picking.cells,
            plates: &picking.plates,
            plate_colors: &self.plate_colors,
            fields: &self.fields,
            arrow_cells,
            arrow_spacing,
            options: self.switcher.options(self.mode),
            abiotic: self.abiotic,
        };
        let mode = self.mode.mode();
        self.displays += 1;
        let vertices = mode
            .colors(&data)
            .and_then(|colors| recolored(&surface.vertices, &colors))
            .map_or_else(|| surface.vertices.clone(), Arc::new);
        let mut builder = OverlayBuilder::default();
        mode.overlay(&data, &mut builder);
        let overlay = (!builder.is_empty()).then(|| builder.finish());
        // The realistic mode draws its own de-indexed mesh with sub-cell detail.
        let detail = (self.mode == RenderModeId::Realistic).then(|| Detail {
            id: self.displays,
            vertices: Arc::new(modes::realistic::detail_vertices(&data, &picking.faces)),
            uniforms: Arc::new(modes::realistic::detail_uniforms(&data)),
        });
        self.display = Some(Surface {
            vertices_id: self.displays,
            vertices,
            detail,
            ..surface.clone()
        });
        self.overlay = overlay.map(|vertices: Vec<OverlayVertex>| Overlay {
            id: self.displays,
            vertices: Arc::new(vertices),
        });
        self.legend = mode.legend(data.options, Some(&data));
        tracing::debug!(
            mode = self.mode.name(),
            elapsed = ?start.elapsed(),
            overlay_vertices = self.overlay.as_ref().map_or(0, |o| o.vertices.len()),
            "Display refreshed"
        );
    }

    /// Whether the active mode waits for fields that have not arrived.
    fn waiting_for_fields(&self) -> bool {
        let needed = self.mode.mode().layers();
        self.surface.is_some() && self.fields.layers().0 & needed.0 != needed.0
    }

    /// Takes a new state of the simulation. The first one of a simulation brings its mesh.
    fn receive(&mut self, mut update: SurfaceUpdate) {
        // Every frame, so that the frames after it, differences to it, can be decoded.
        if let (Some(history), Some(frame)) = (&mut self.history, update.frame.take()) {
            history.record(frame);
        }
        if let Some(mesh) = &update.mesh {
            self.set_mesh(mesh);
        }
        if self.picking.is_none() {
            tracing::error!("Surface update without a mesh");
            return;
        }
        // A state sent again (with other fields) is not the step asked for.
        let newer = self.time.is_none_or(|time| update.time > time);
        // The step the user asked for shows at once, not at the pace of the view.
        let asked = newer && self.step_pending;
        if newer {
            self.step_pending = false;
            // The simulation stops after the step it was asked for; "pausing" only if it does
            // not report it soon (it writes the step's snapshot first).
            self.request_time = Some(web_time::Instant::now());
        }
        self.time = Some(update.time);
        self.global = Some(update.global);
        self.pace.record(update.step_seconds);
        if self.finished() {
            // The simulation does not hold its last state.
            self.paused = false;
        }
        // Colours for new plates before any frame with them can be shown again.
        if let Some(picking) = &self.picking {
            self.plate_colors
                .update(&picking.mesh, &update.cells, &update.plates);
        }
        self.latest = Some(Latest {
            time: update.time,
            global: update.global,
            cells: update.cells.clone(),
            plates: update.plates.clone(),
            fields: update.fields.clone(),
        });
        if self.viewing.is_none() {
            // The first state of a simulation, the newest state sent again with the fields of a
            // new mode and an asked-for step show at once; others at the pace of the view. A
            // state that is not due yet makes the view fall behind: it walks on through the
            // recorded frames from the shown one, every state in turn, and is live again once it
            // catches up (Live or End jumps to the newest at once).
            let now = web_time::Instant::now();
            let first = update.mesh.is_some();
            let resent = !first && !newer;
            if first || resent || asked || self.walk.due_in(now).is_zero() {
                if !resent {
                    self.walk.shown(now);
                }
                self.live_pending = false;
                self.shown_global = Some(update.global);
                self.show(update.time, update.cells, update.plates, update.fields);
            } else if let Some(index) = self.recorded_index(self.shown_time) {
                self.viewing = Some(index);
                self.shown_frame = Some(index);
                self.walk.fall_behind();
            } else {
                // The shown state was not recorded (the history is full): the newest state at
                // the next tick.
                self.live_pending = true;
            }
        }
    }

    /// The recorded frame of the state at `time`, if it was recorded.
    fn recorded_index(&self, time: f64) -> Option<usize> {
        let history = self.history.as_ref()?;
        (0..history.len())
            .rev()
            .map(|index| (index, history.time(index)))
            .take_while(|(_, frame)| frame.is_some_and(|frame| frame >= time))
            .find(|(_, frame)| *frame == Some(time))
            .map(|(index, _)| index)
    }

    /// Takes the mesh of a new simulation.
    fn set_mesh(&mut self, update: &MeshUpdate) {
        self.meshes += 1;
        // A new simulation: its plates are others.
        self.plate_colors.clear();
        let directions: Vec<DVec3> = update
            .directions
            .iter()
            .map(|&direction| Vec3::from(direction).as_dvec3().normalize_or(DVec3::X))
            .collect();
        // 10 · 4^s + 2 vertices for s subdivisions.
        let subdivisions = (directions.len().saturating_sub(2) / 10).max(1).ilog2() / 2;
        let mesh = SphereMesh::from_faces(directions, update.faces.clone(), subdivisions);
        // The selected place stays selected on the new mesh (another resolution).
        self.selected = match (self.selected, &self.picking) {
            (Some(cell), Some(old)) if (cell as usize) < old.mesh.len() => {
                Some(mesh.nearest(old.mesh.vertex(cell)))
            }
            _ => None,
        };
        self.hovered = None;
        self.picking = Some(Picking {
            mesh_id: self.meshes,
            mesh,
            faces: Arc::new(update.faces.clone()),
            cells: Arc::new(Vec::new()),
            drainage: hydrology::Drainage::default(),
            plates: Arc::new(Vec::new()),
            max_radius: 1.0,
        });
    }

    /// Shows a state of the current mesh: builds its surface and takes its cells for picking.
    fn show(
        &mut self,
        time: f64,
        cells: Arc<Vec<CellInfo>>,
        plates: Arc<Vec<PlateInfo>>,
        fields: Fields,
    ) {
        let exaggeration = self.shown_exaggeration();
        let Some(picking) = &mut self.picking else {
            return;
        };
        if cells.len() != picking.mesh.len() {
            tracing::error!(
                cells = cells.len(),
                mesh = picking.mesh.len(),
                "Cell attributes of the wrong size"
            );
            return;
        }
        let vertices = surface_vertices(&picking.mesh, &cells, exaggeration, self.river_threshold);
        picking.max_radius = vertices
            .iter()
            .map(|vertex| f64::from(Vec3::from(vertex.position).length()))
            .fold(1.0, f64::max);
        picking.drainage = hydrology::Drainage::of(&picking.mesh, &cells);
        picking.cells = cells;
        picking.plates = plates;
        self.shown_time = time;
        self.surfaces += 1;
        self.surface = Some(Surface {
            mesh_id: picking.mesh_id,
            faces: picking.faces.clone(),
            vertices_id: self.surfaces,
            vertices: Arc::new(vertices),
            detail: None,
        });
        self.fields = fields;
        self.display_outdated = true;
    }

    /// Shows a past frame that the history decoded: the requested one, or one requested before
    /// it while the time bar is dragged (see [`History::poll`]).
    fn poll_history(&mut self) {
        let Some(result) = self.history.as_mut().and_then(History::poll) else {
            return;
        };
        match result {
            Ok(frame) if self.viewing.is_some() => {
                let Some(picking) = &self.picking else {
                    self.walk.stop();
                    return;
                };
                if frame.cells.len() != picking.mesh.len() {
                    self.walk.stop();
                    self.frame_error = Some("the frame does not fit the mesh".to_owned());
                    return;
                }
                let fields = Fields::from_frame(&picking.mesh, &frame.cells, &frame.plates);
                let time = self
                    .history
                    .as_ref()
                    .and_then(|history| history.time(frame.index))
                    .unwrap_or(self.shown_time);
                self.frame_error = None;
                if let Some(global) = self
                    .history
                    .as_ref()
                    .and_then(|history| history.global(frame.index))
                {
                    self.shown_global = Some(global);
                }
                self.show(time, Arc::new(frame.cells), Arc::new(frame.plates), fields);
                self.shown_frame = Some(frame.index);
                // The walk holds every frame for its interval from when it is shown.
                if self.walk.is_playing() {
                    self.walk.shown(web_time::Instant::now());
                }
            }
            Ok(_) => {}
            // A read that failed after the view went live again is no longer of interest.
            Err(_) if self.viewing.is_none() => {}
            Err(error) => {
                // The frames after a lost one are lost as well: no use walking on.
                self.walk.stop();
                self.frame_error = Some(format!("Cannot show this frame: {error}"));
            }
        }
    }

    /// The positions of the time bar: the recorded frames, and the newest state if it is
    /// newer than the last of them (when the history stopped recording).
    fn positions(&self) -> usize {
        let (Some(history), Some(latest)) = (&self.history, &self.latest) else {
            return 0;
        };
        let recorded = history.len();
        let newer = recorded
            .checked_sub(1)
            .and_then(|last| history.time(last))
            .is_none_or(|time| latest.time > time);
        recorded + usize::from(newer)
    }

    /// Shows the recorded frame `index`, or the newest state if it is the last position.
    fn view(&mut self, index: usize) {
        if index + 1 >= self.positions() {
            self.go_live();
            return;
        }
        self.hold(index);
    }

    /// Shows the recorded frame `index` and stays on it, even if it is the newest.
    fn hold(&mut self, index: usize) {
        self.live_pending = false;
        self.viewing = Some(index);
        if let Some(history) = &mut self.history {
            history.request(index);
        }
        self.poll_history();
    }

    /// Pauses a view that follows the newest state: it stays on the shown state (the pace of
    /// the view may keep it behind the newest), read from the history as a recorded frame, so
    /// that it has every field (`Fields::from_frame`) like any other past frame. With a full
    /// history (the shown state may not be recorded) that is the last recorded frame at or
    /// before it.
    fn freeze(&mut self) {
        let Some(history) = &self.history else {
            return;
        };
        let Some(last) = history.len().checked_sub(1) else {
            return;
        };
        // The shown state, which the pace of the view may keep behind the newest one.
        let shown = (0..=last)
            .rev()
            .find(|&index| {
                history
                    .time(index)
                    .is_some_and(|time| time <= self.shown_time)
            })
            .unwrap_or(last);
        self.walk.stop();
        self.hold(shown);
    }

    /// Walks a playing view on to the next frame once it is due, or shows the newest state of
    /// a live one, and asks for a repaint when the next will be.
    fn walk(&mut self, ctx: &egui::Context) {
        let Some(index) = self.viewing else {
            if self.live_pending {
                let wait = self.walk.due_in(web_time::Instant::now());
                if wait.is_zero() {
                    self.show_latest();
                } else {
                    ctx.request_repaint_after(wait);
                }
            }
            return;
        };
        let now = web_time::Instant::now();
        let Some(wait) = self.walk.wait(now, self.shown_frame == Some(index)) else {
            return;
        };
        if wait.is_zero() {
            // On the last position this goes live: from then on the view follows the
            // simulation at its own pace.
            self.view(index + 1);
            if self.viewing.is_some() && self.shown_frame == self.viewing {
                // Read at once (the frame was decoded before): the next one is due later.
                ctx.request_repaint_after(self.walk.interval());
            }
        } else {
            ctx.request_repaint_after(wait);
        }
    }

    /// Follows the newest state again, showing it at once.
    fn go_live(&mut self) {
        self.walk.stop();
        self.shown_frame = None;
        self.frame_error = None;
        if let Some(history) = &mut self.history {
            history.cancel();
        }
        if self.viewing.take().is_some() || self.live_pending {
            self.show_latest();
        }
    }

    /// Shows the newest state, which starts the next interval of the view's pace.
    fn show_latest(&mut self) {
        self.live_pending = false;
        self.walk.shown(web_time::Instant::now());
        if let Some(latest) = &self.latest {
            self.shown_global = Some(latest.global);
            let (time, cells, plates, fields) = (
                latest.time,
                latest.cells.clone(),
                latest.plates.clone(),
                latest.fields.clone(),
            );
            self.show(time, cells, plates, fields);
        }
    }

    /// Whether the running simulation is complete (it takes `round(duration / dt)` steps,
    /// like `crate::Run`), or failed.
    fn finished(&self) -> bool {
        self.failure.is_some()
            || self
                .time
                .is_some_and(|time| crate::is_last_step(time, self.duration, self.dt))
    }

    /// Carries out an action of the time bar.
    fn time_action(&mut self, action: timeline::TimeAction) {
        use timeline::TimeAction;
        let positions = self.positions();
        let position = self.viewing.unwrap_or(positions.saturating_sub(1));
        match action {
            TimeAction::TogglePlay => {
                if self.viewing.is_none() {
                    self.freeze();
                } else if self.walk.is_playing() {
                    self.walk.stop();
                } else if self.frame_error.is_some() {
                    // The frames after one that cannot be read cannot be either (see
                    // `History`): play on from the newest state.
                    self.go_live();
                } else {
                    self.walk.play();
                }
                tracing::info!(
                    playing = self.viewing.is_none() || self.walk.is_playing(),
                    "View playing"
                );
            }
            TimeAction::ToggleSimulation => {
                if self.finished() {
                    return;
                }
                self.paused = !self.paused;
                if !self.paused {
                    self.step_pending = false;
                }
                tracing::info!(paused = self.paused, "Simulation paused");
                self.requests += 1;
                self.halted = None;
                self.request_time = self.paused.then(web_time::Instant::now);
                if let Some(control) = &mut self.control {
                    control.set_paused(self.paused, self.requests);
                }
            }
            TimeAction::Back => {
                if position > 0 && positions > 0 {
                    self.walk.stop();
                    self.view(position - 1);
                }
            }
            TimeAction::Forward => {
                if position + 1 < positions {
                    self.walk.stop();
                    self.view(position + 1);
                } else if self.paused && !self.finished() && !self.step_pending {
                    // On the newest frame (live or paused on it): show the step when it comes.
                    self.go_live();
                    self.step_pending = true;
                    self.requests += 1;
                    self.halted = None;
                    self.request_time = Some(web_time::Instant::now());
                    if let Some(control) = &mut self.control {
                        control.step(self.requests);
                    }
                } else if self.viewing.is_some() {
                    // Paused on the newest frame: follow the simulation again.
                    self.go_live();
                }
            }
            TimeAction::Seek(index) => {
                self.walk.stop();
                self.view(index);
            }
            TimeAction::Live => self.go_live(),
        }
    }

    /// What the paused simulation is doing, for the time bar: "pausing" only once the
    /// simulation has not answered for [`PAUSING_DELAY`] (it usually answers at once).
    fn simulation_status(&self, ctx: &egui::Context) -> Option<timeline::Status> {
        if !self.paused || self.finished() {
            return None;
        }
        if self.step_pending {
            return Some(timeline::Status::Stepping);
        }
        match self.halted {
            Some(true) => Some(timeline::Status::MidStep),
            Some(false) => None,
            None => {
                let waited = self
                    .request_time
                    .map_or(PAUSING_DELAY, |time| time.elapsed());
                if waited >= PAUSING_DELAY {
                    Some(timeline::Status::Pausing)
                } else {
                    ctx.request_repaint_after(PAUSING_DELAY - waited);
                    None
                }
            }
        }
    }

    /// Shows the time bar (once the simulation sent a state) and returns the area it covers.
    fn time_bar(&mut self, ctx: &egui::Context) -> Option<egui::Rect> {
        let (Some(history), Some(latest)) = (&self.history, &self.latest) else {
            return None;
        };
        let positions = self.positions();
        let position = self.viewing.unwrap_or(positions.saturating_sub(1));
        let recorded = history.len();
        let time_of = |index: usize| {
            if index < recorded {
                history.time(index)
            } else {
                Some(latest.time)
            }
        };
        let (stored, raw) = history.bytes();
        let mut about = format!(
            "{recorded} frames kept {}: {:.1} MB ({:.1} MB before compression)",
            history.describe(),
            stored as f64 / 1e6,
            raw as f64 / 1e6
        );
        if history.is_full() {
            about.push_str("; full, later states are not kept");
        }
        let view = timeline::TimelineView {
            positions,
            position,
            live: self.viewing.is_none(),
            time_of: &time_of,
            live_time: latest.time,
            live_shown: self.shown_time,
            rate: self.walk.rate,
            playing: self.viewing.is_none() || self.walk.is_playing(),
            paused: self.paused,
            finished: self.finished(),
            step_pending: self.step_pending,
            status: self.simulation_status(ctx),
            loading: self.viewing.is_some() && history.is_loading(),
            error: self.frame_error.as_deref(),
            about,
        };
        let key = timeline::keys(ctx);
        let (clicked, rect) = self.time_bar.show(ctx, &view);
        if let Some(open) = self.time_bar.take_choice() {
            settings::set_bool(self.settings.as_mut(), timeline::STORAGE_KEY, open);
        }
        if let Some(action) = clicked.or(key) {
            self.time_action(action);
            ctx.request_repaint();
        }
        Some(rect)
    }

    /// The cell under a point of the view `rect` (in points), if the planet is there.
    fn pick(
        &self,
        view_projection: glam::Mat4,
        rect: egui::Rect,
        point: egui::Pos2,
    ) -> Option<u32> {
        let picking = self.picking.as_ref()?;
        let vertices = &self.surface.as_ref()?.vertices;
        let ndc = Vec2::new(
            (point.x - rect.left()) / rect.width().max(1.0) * 2.0 - 1.0,
            1.0 - (point.y - rect.top()) / rect.height().max(1.0) * 2.0,
        );
        let ray = picking::pointer_ray(view_projection, ndc);
        picking::pick(
            &picking.mesh,
            |cell| f64::from(Vec3::from(vertices[cell as usize].position).length()),
            picking.max_radius,
            &ray,
            self.hovered.or(self.selected),
        )
    }

    /// The outline of a cell as line segments for the renderer, `pixels_per_point` physical
    /// pixels per point.
    fn outline(&self, cell: u32, pixels_per_point: f32) -> Vec<LineSegment> {
        let (Some(picking), Some(surface)) = (&self.picking, &self.surface) else {
            return Vec::new();
        };
        let eye = self.camera_position().as_dvec3();
        let lift = OUTLINE_LIFT * picking.mesh.mean_edge_angle();
        let points: Vec<[f32; 3]> = picking::outline(
            &picking.mesh,
            |vertex| Vec3::from(surface.vertices[vertex as usize].position).as_dvec3(),
            cell,
        )
        .into_iter()
        .map(|point| {
            (point + (eye - point).normalize_or_zero() * lift)
                .as_vec3()
                .to_array()
        })
        .collect();
        let points = &points;
        let segments = |width: f32, color: [f32; 4]| {
            (0..points.len()).map(move |index| LineSegment {
                start: points[index],
                width: width * pixels_per_point,
                end: points[(index + 1) % points.len()],
                _padding: 0.0,
                color,
            })
        };
        // The dark border first, the light line over it.
        segments(OUTLINE_BORDER_WIDTH, [0.0, 0.0, 0.0, 1.0])
            .chain(segments(OUTLINE_WIDTH, [1.0, 1.0, 1.0, 1.0]))
            .collect()
    }

    fn camera_position(&self) -> Vec3 {
        // The planet's rotation axis is +z.
        Vec3::new(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
        ) * CAMERA_DISTANCE
    }

    /// The view-projection matrix and the vertical field of view for a view of `viewport`
    /// physical pixels.
    fn view_projection(&self, viewport: egui::Vec2) -> (glam::Mat4, f32) {
        let aspect_ratio = viewport.x / viewport.y.max(1.0);
        let eye = self.camera_position();
        let view = glam::camera::rh::view::look_at_mat4(eye, Vec3::ZERO, Vec3::Z);
        // wgpu uses normalised device coordinates with Z in [0, 1] and Y up.
        let field_of_view = vertical_field_of_view(aspect_ratio);
        let projection =
            glam::camera::rh::proj::directx::perspective(field_of_view, aspect_ratio, 0.1, 10.0);
        (projection * view, field_of_view)
    }

    /// Meters the view of `viewport` physical pixels and lets the exposure adapt to it over the
    /// wall-clock time since the last frame. Returns the metering (only with the sun's light;
    /// the camera light keeps the daylight exposure) and whether the exposure still moves, so
    /// that the view needs another frame.
    fn adapt(&mut self, viewport: egui::Vec2) -> (Option<sky::Exposure>, bool) {
        let now = web_time::Instant::now();
        let seconds = self
            .adapted_at
            .replace(now)
            .map_or(0.0, |then| now.duration_since(then).as_secs_f32());
        if !self.sky_shown() {
            self.adaptation = sky::Adaptation::daylight();
            return (None, false);
        }
        let (view_projection, _) = self.view_projection(viewport);
        let sun = self.sun();
        let exposure = sky::Exposure::new(
            view_projection,
            self.camera_position(),
            sun.direction,
            sun.angular_radius,
            self.atmosphere_samples() > 0,
        );
        let moving = self.adaptation.update(exposure.target(), seconds);
        (Some(exposure), moving)
    }

    /// The uniforms for a view of `viewport` physical pixels, with the metering of the sky
    /// (`None` with the camera light).
    fn uniforms(&self, viewport: egui::Vec2, exposure: Option<sky::Exposure>) -> Uniforms {
        let eye = self.camera_position();
        let (view_projection, field_of_view) = self.view_projection(viewport);
        let sun = self.sun();
        let lighting = Lighting::new(self.light(), eye, &sun);
        // The exposure brightens the direct sunlight on the planet, not the ambient fill (a
        // visual aid, see `lighting`); with the camera light it stays at 1.
        let gain = if exposure.is_some() {
            self.adaptation.srgb_gain()
        } else {
            1.0
        };
        Uniforms {
            view_projection: view_projection.to_cols_array_2d(),
            light_direction: lighting.direction.extend(0.0).to_array(),
            viewport: [
                viewport.x,
                viewport.y,
                match self.mode.mode().shading() {
                    Shading::Lit => 0.0,
                    Shading::Unlit => 1.0,
                },
                self.atmosphere_samples() as f32,
            ],
            inverse_view_projection: view_projection.inverse().to_cols_array_2d(),
            sun_direction: sun.direction.extend(sun.angular_radius).to_array(),
            sun_color: [
                sun.disc_color[0],
                sun.disc_color[1],
                sun.disc_color[2],
                exposure.map_or(1.0, |exposure| exposure.sun_visible),
            ],
            light_color: [
                lighting.color[0] * gain,
                lighting.color[1] * gain,
                lighting.color[2] * gain,
                lighting.ambient,
            ],
            stars: [
                self.adaptation.star_dimming(),
                field_of_view / viewport.y.max(1.0),
                gain,
                if self.sky_shown() { 1.0 } else { 0.0 },
            ],
            star_colors: sky::star_colors(),
            eye: eye.extend(0.0).to_array(),
        }
    }

    /// The relief exaggeration of the shown surface: true to scale in the realistic mode
    /// (decision 6 on #92), `--exaggeration` in the data modes.
    fn shown_exaggeration(&self) -> f64 {
        match self.mode {
            RenderModeId::Realistic => 1.0,
            _ => self.exaggeration,
        }
    }

    /// Where the light comes from: the realistic mode's choice, the camera in the data modes.
    fn light(&self) -> Light {
        match self.mode {
            RenderModeId::Realistic => modes::realistic::light(self.switcher.options(self.mode)),
            _ => Light::Camera,
        }
    }

    /// The number of points along a ray of the realistic mode's atmosphere, 0 without one (the
    /// data modes, or switched off).
    fn atmosphere_samples(&self) -> u32 {
        match self.mode {
            RenderModeId::Realistic => {
                modes::realistic::atmosphere_samples(self.switcher.options(self.mode))
            }
            _ => 0,
        }
    }

    /// Whether the clouds drift (#187): in the realistic mode with the clouds on at detail High.
    /// The view then redraws at the rate of [`modes::realistic::drift_repaint`].
    fn clouds_drift(&self) -> bool {
        self.mode == RenderModeId::Realistic
            && modes::realistic::clouds_drift(self.switcher.options(self.mode))
            // Only once the surface with clouds is there (not while it loads).
            && self
                .display
                .as_ref()
                .and_then(|display| display.detail.as_ref())
                .is_some_and(|detail| detail.uniforms.clouds[0] > 0.0)
    }

    /// Whether the sky (the sun and the stars) is drawn: only with the sun's light.
    fn sky_shown(&self) -> bool {
        self.light() != Light::Camera
    }

    /// The sun of the displayed state.
    fn sun(&self) -> SunView {
        let season = match self.mode {
            RenderModeId::Realistic => lighting::season_fraction(modes::realistic::season(
                self.switcher.options(self.mode),
            )),
            _ => None,
        };
        SunView::new(self.epoch, self.shown_time, season)
    }
}

/// The vertical field of view for a view of this aspect ratio: [`FIELD_OF_VIEW`], or in
/// portrait (phones) wider, so that it applies horizontally and the planet fits.
fn vertical_field_of_view(aspect_ratio: f32) -> f32 {
    if aspect_ratio < 1.0 {
        2.0 * ((FIELD_OF_VIEW / 2.0).tan() / aspect_ratio.max(0.01)).atan()
    } else {
        FIELD_OF_VIEW
    }
}

/// Points per planet radius at the point of the planet nearest to the camera, in a view of
/// `size` points.
fn points_per_radius(size: egui::Vec2) -> f32 {
    let field_of_view = vertical_field_of_view(size.x / size.y.max(1.0));
    size.y / (2.0 * (CAMERA_DISTANCE - 1.0) * (field_of_view / 2.0).tan())
}

impl eframe::App for Viewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame_times.frame(self.mode);
        let mut updated = false;
        while let Ok(event) = self.events.try_recv() {
            match event {
                SimulationEvent::Surface(update) => self.receive(update),
                SimulationEvent::Generating => self.generating = true,
                SimulationEvent::Failed(message) => self.failure = Some(message),
                SimulationEvent::Halted(halt) => {
                    tracing::info!(?halt, "Simulation halted");
                    // Not if a later request overtook it.
                    if self.paused && halt.request == self.requests {
                        self.halted = Some(halt.in_step);
                    }
                }
            }
            updated = true;
        }
        self.poll_history();
        self.walk(ui.ctx());
        if updated && let Some(time) = self.time {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(format!(
                    "World Simulator — {time:.0} Myr"
                )));
        }

        let mut shown = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(5, 5, 15)))
            .show(ui, |ui| {
                // Dragging rotates the planet; a click or tap (a press that does not move
                // further than egui's click distance) selects a cell.
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                // egui starts a drag of a widget that also senses clicks only once the pointer
                // moved further than a click may; the rotation still follows it from the press.
                let delta = if response.drag_started() {
                    ui.input(|input| {
                        input
                            .pointer
                            .press_origin()
                            .zip(input.pointer.interact_pos())
                            .map(|(origin, now)| now - origin)
                    })
                    .unwrap_or_else(|| response.drag_delta())
                } else {
                    response.drag_delta()
                };
                self.yaw -= delta.x * ROTATION_SPEED;
                self.pitch = (self.pitch + delta.y * ROTATION_SPEED).clamp(-1.5, 1.5);

                let pixels_per_point = ui.ctx().pixels_per_point();
                let viewport = rect.size() * pixels_per_point;
                let (exposure, adapting) = self.adapt(viewport);
                if adapting {
                    // Keep drawing while the exposure adapts, also with the simulation paused
                    // and the camera still; stop once it has converged.
                    ui.ctx().request_repaint();
                }
                let uniforms = self.uniforms(viewport, exposure);
                self.points_per_radius = points_per_radius(rect.size());
                self.refresh_display();
                let view_projection = glam::Mat4::from_cols_array_2d(&uniforms.view_projection);
                if response.clicked()
                    && let Some(point) = response.interact_pointer_pos()
                {
                    // A click beside the planet clears the selection.
                    self.selected = self.pick(view_projection, rect, point);
                }
                // The cell under a hovering mouse; none while a button is down (dragging) or
                // on touch screens after the finger is lifted (egui then has no pointer).
                let pointing = ui.input(|input| input.pointer.any_down());
                self.hovered = response
                    .hover_pos()
                    .filter(|_| !pointing)
                    .and_then(|point| self.pick(view_projection, rect, point));
                shown = self
                    .hovered
                    .map(|cell| (cell, false))
                    .or(self.selected.map(|cell| (cell, true)));

                let lines = shown
                    .map(|(cell, _)| self.outline(cell, pixels_per_point))
                    .unwrap_or_default();
                // Drifting clouds move on the display's clock, redrawn at a modest rate.
                let cloud_drift = self.clouds_drift().then(|| {
                    ui.ctx()
                        .request_repaint_after(modes::realistic::drift_repaint(
                            self.points_per_radius * pixels_per_point,
                        ));
                    modes::realistic::cloud_drift(ui.input(|input| input.time))
                });
                ui.painter()
                    .add(eframe::egui_wgpu::Callback::new_paint_callback(
                        rect,
                        PlanetCallback {
                            uniforms,
                            surface: self.display.clone(),
                            overlay: self.overlay.clone(),
                            lines,
                            sky: self.sky_shown() || self.atmosphere_samples() > 0,
                            cloud_drift,
                        },
                    ));
            });

        // Over the planet, below the parameter panel (see `banner`).
        let restarted = self.runs > 1;
        if let Some(text) = banner::simulation_text(
            restarted,
            self.generating,
            self.time,
            self.failure.as_deref(),
        ) {
            banner::show(ui, &text);
        }

        // The time bar along the bottom edge, the cell widget above it.
        let time_bar = self.time_bar(ui.ctx());
        let shown = shown.and_then(|(cell, selected)| {
            let picking = self.picking.as_ref()?;
            let info = picking.cells.get(cell as usize)?;
            Some(ShownCell {
                cell,
                info,
                plate_id: picking
                    .plates
                    .get(info.plate as usize)
                    .map(|plate| plate.id),
                direction: picking.mesh.vertex(cell),
                basin: picking
                    .drainage
                    .describe(&picking.mesh, &picking.cells, cell),
                selected,
            })
        });
        let above_bar = time_bar.map_or(0.0, |rect| ui.ctx().content_rect().bottom() - rect.top());
        let cell_panel = self.cell_panel.show(ui.ctx(), shown, self.touch, above_bar);

        let mut mode = self.mode;
        let view = SwitcherView {
            legend: &self.legend,
            points_per_radius: self.points_per_radius,
            waiting: self.waiting_for_fields(),
            bottom: cell_panel.map_or(f32::INFINITY, |rect| rect.top()),
            sun: (self.mode == RenderModeId::Realistic).then(|| self.sun().describe()),
        };
        let (changed, switcher) = self.switcher.show(ui.ctx(), &mut mode, &view);
        if changed {
            if mode != self.mode {
                self.set_mode(mode);
            } else {
                // Another setting of the mode (in the realistic mode the light only needs the
                // uniforms, but the season changes the colours).
                self.display_outdated = true;
            }
            ui.ctx().request_repaint();
        }

        // The global states of the recorded frames for the panel's sparklines (#209).
        let latest = self
            .latest
            .as_ref()
            .map(|latest| (latest.time, latest.global));
        let series = match &self.history {
            Some(history) => global::thin(history.globals(), latest),
            None => global::thin(std::iter::empty(), latest),
        };
        let status = SimulationStatus {
            time: self.time,
            duration: self.duration,
            failure: self.failure.as_deref(),
            starting: banner::starting_text(restarted, self.generating),
            step_seconds: self.pace.median(),
            global: self.shown_global,
            shown_time: self.shown_time,
            epoch: self.epoch,
            series,
        };
        let pace = &mut self.pace;
        let mut hint_action = None;
        let hint = pace
            .has_hint(self.time, self.duration)
            .then_some(|ui: &mut egui::Ui| {
                hint_action = pace.ui(ui, self.time, self.duration);
            });
        let mut launch =
            self.parameters
                .show(ui.ctx(), &status, cell_panel, time_bar, switcher, hint);
        if let Some(HintAction::Restart(subdivisions)) = hint_action {
            match self.parameters.launch_with_subdivisions(subdivisions) {
                Ok(new) => {
                    self.parameters.close_after_restart(ui.ctx());
                    launch = Some(new);
                }
                Err(error) => {
                    tracing::error!("Cannot restart with the proposed subdivisions: {error}")
                }
            }
        }
        if let Some(launch) = launch {
            tracing::info!(args = ?launch.args, "Restarting the simulation");
            self.parameters.set_running(launch.args.clone());
            let ctx = ui.ctx().clone();
            self.start(&ctx, launch);
        }

        if let Some(on_first_frame) = self.on_first_frame.take() {
            on_first_frame();
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn lakes_stand_at_their_water_level_and_dry_depressions_below_the_sea() {
        let mesh = SphereMesh::icosphere(1);
        let mut cells = vec![<CellInfo as bytemuck::Zeroable>::zeroed(); mesh.len()];
        let base = |elevation: f32, drainage: u32, lake_depth: f32| CellInfo {
            elevation,
            drainage,
            lake_depth,
            ..<CellInfo as bytemuck::Zeroable>::zeroed()
        };
        cells[0] = base(
            -4000.0,
            CellInfo::drainage_bits(None, true, false, 0.0),
            0.0,
        );
        cells[1] = base(
            -120.0,
            CellInfo::drainage_bits(Some(0), false, true, 50.0),
            50.0,
        );
        cells[2] = base(
            -120.0,
            CellInfo::drainage_bits(Some(0), false, true, 0.5),
            0.5,
        );
        cells[3] = base(
            300.0,
            CellInfo::drainage_bits(Some(0), false, false, 0.0),
            0.0,
        );
        let vertices = surface_vertices(&mesh, &cells, 40.0, 1e30);
        let radius = |cell: usize| f64::from(glam::Vec3::from(vertices[cell].position).length());
        let scale = 40.0 / crate::EARTH_RADIUS;
        assert!((radius(0) - 1.0).abs() < 1e-6);
        assert!((radius(1) - (1.0 - 70.0 * scale)).abs() < 1e-6);
        assert!((radius(2) - (1.0 - 120.0 * scale)).abs() < 1e-6);
        assert!((radius(3) - (1.0 + 300.0 * scale)).abs() < 1e-6);
        let color = |cell: usize| {
            let [r, g, b, _] = vertices[cell].color.map(|c| (c * 255.0).round() as u8);
            [r, g, b]
        };
        assert_eq!(color(1), render::lake_color(50.0).0);
        assert_eq!(color(2), render::BELOW_SEA_LEVEL_COLOR);
    }

    #[test]
    fn window_icon_decodes() {
        let icon = super::window_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }
}
