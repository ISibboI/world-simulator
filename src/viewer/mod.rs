//! Interactive 3D view of the planet.
//!
//! The window is an [`eframe`] (egui) application, which runs on Windows, macOS and Linux, and
//! lets later user interface elements be drawn on top of the 3D view. The planet itself is
//! drawn with wgpu through an egui paint callback (see [`renderer`]).
//!
//! The simulation runs on a separate thread (in a Web Worker on the web, see `crate::web`) and
//! sends new surface meshes to the view through a channel, each with the attributes of every
//! cell ([`CellInfo`]). The parameter panel ([`parameters`]) restarts it with other options; the
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
mod banner;
mod cell_info;
mod fields;
mod modes;
mod pace;
mod parameters;
mod picking;
mod renderer;

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
pub use fields::{Fields, Layers};
pub use modes::RenderModeId;
use modes::switcher::{ModeSwitcher, SwitcherView};
use modes::{ModeData, recolored};
#[cfg(target_arch = "wasm32")]
pub use modes::{STORAGE_KEY as RENDER_MODE_STORAGE_KEY, initial as initial_render_mode, to_store};
use pace::{HintAction, Pace};
use parameters::{ParameterPanel, SimulationStatus};
#[cfg(target_arch = "wasm32")]
pub use parameters::{has_option, with_default};
#[cfg(target_arch = "wasm32")]
pub use renderer::Vertex;
#[cfg(not(target_arch = "wasm32"))]
use renderer::Vertex;
use renderer::{LineSegment, Overlay, PlanetCallback, Surface, Uniforms};

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
    pub vertices: Arc<Vec<Vertex>>,
    /// The attributes of every cell, in the order of the vertices.
    pub cells: Arc<Vec<CellInfo>>,
    /// The triangles of the mesh: sent with the first update of a simulation, `None` after.
    pub faces: Option<Arc<Vec<[u32; 3]>>>,
    /// Seconds since the previous update, i.e. the time of the step: `None` for the initial
    /// state, for a repeated state (sent again with other fields), or if it is not
    /// representative (the web page was hidden meanwhile).
    pub step_seconds: Option<f64>,
    /// The extra fields the view requested (see [`SimulationControl::request_layers`]).
    pub fields: Fields,
}

/// A message from the simulation to the view.
pub enum SimulationEvent {
    /// The simulation is generating its initial planet (it sends no surface before).
    Generating,
    Surface(SurfaceUpdate),
    /// The simulation failed with this message.
    Failed(String),
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

/// Builds the surface mesh vertices for the current state of the planet.
///
/// Land is raised above the sphere by its elevation times `exaggeration`; the ocean surface is at
/// radius 1.
pub fn surface_vertices(planet: &Planet, exaggeration: f64, river_threshold: f64) -> Vec<Vertex> {
    let simulation = planet.tectonics();
    let mesh = simulation.mesh();
    let elevation = simulation.surface_elevations();
    let radius = crate::EARTH_RADIUS;

    let positions: Vec<glam::DVec3> = (0..mesh.len() as u32)
        .map(|cell| {
            let height = elevation[cell as usize].max(0.0) * exaggeration / radius;
            mesh.vertex(cell) * (1.0 + height)
        })
        .collect();

    let mut normals = vec![glam::DVec3::ZERO; mesh.len()];
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

    (0..mesh.len())
        .map(|cell| {
            let image::Rgb(color) = render::surface_color(planet, cell as u32, river_threshold);
            Vertex {
                position: positions[cell].as_vec3().to_array(),
                normal: normals[cell].normalize_or_zero().as_vec3().to_array(),
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
    /// simulation sends its current state again with them if they include new ones, as soon
    /// as it can (between steps, or at once once it has finished), so that a new mode does not
    /// wait for the next step.
    fn request_layers(&mut self, layers: Layers);
}

/// Called when the user chose another render mode, with the command-line arguments of the
/// running simulation with that mode (the web page puts them into its URL and remembers the
/// mode).
pub type RememberRenderMode = Box<dyn FnMut(RenderModeId, &[String])>;

/// Creates the viewer app; shared by the desktop window and the web page.
fn app_creator(
    initial: Launch,
    msaa_samples: u16,
    ui_options: UiOptions,
    start_simulation: StartSimulation,
    remember_render_mode: Option<RememberRenderMode>,
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
            remember_render_mode,
            on_first_frame,
        )))
    })
}

/// Opens the 3D view and blocks until the window is closed.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(initial: Launch, start_simulation: StartSimulation) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("World Simulator")
            .with_inner_size([1200.0, 900.0]),
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
            None,
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
pub async fn run_web(
    canvas: web_sys::HtmlCanvasElement,
    initial: Launch,
    ui_options: UiOptions,
    start_simulation: StartSimulation,
    remember_render_mode: RememberRenderMode,
    on_first_frame: Box<dyn FnOnce()>,
) -> anyhow::Result<()> {
    let options = eframe::WebOptions {
        depth_buffer: DEPTH_BUFFER_BITS,
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
                Some(remember_render_mode),
                Some(on_first_frame),
            ),
        )
        .await
        .map_err(|error| anyhow::anyhow!("failed to start the 3D view: {error:?}"))
}

struct Viewer {
    events: Receiver<SimulationEvent>,
    start_simulation: StartSimulation,
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
    parameters: ParameterPanel,
    cell_panel: CellInfoPanel,
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
    /// Simulated time of the displayed state of the running simulation.
    time: Option<f64>,
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
    /// The latest surface; the previous simulation's until the new one sends its first.
    surface: Option<Surface>,
    /// The number of meshes and surface states received, for [`Surface`]'s ids.
    meshes: u64,
    surfaces: u64,
}

/// What picking needs to know about the displayed surface.
struct Picking {
    /// [`Surface::mesh_id`] of the mesh.
    mesh_id: u64,
    /// The mesh with the directions of the displayed vertices.
    mesh: SphereMesh,
    /// The attributes of the cells of the displayed state.
    cells: Arc<Vec<CellInfo>>,
    /// The largest displayed radius of a vertex.
    max_radius: f64,
}

impl Viewer {
    fn new(
        ctx: &egui::Context,
        initial: Launch,
        ui_options: UiOptions,
        start_simulation: StartSimulation,
        remember_render_mode: Option<RememberRenderMode>,
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
            parameters,
            cell_panel: CellInfoPanel::default(),
            pace,
            touch: ui_options.touch,
            picking: None,
            selected: None,
            hovered: None,
            yaw: 0.0,
            pitch: 0.4,
            time: None,
            duration: initial.cli.duration,
            failure: None,
            runs: 0,
            generating: false,
            on_first_frame,
            surface: None,
            meshes: 0,
            surfaces: 0,
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
        self.duration = launch.cli.duration;
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
        self.mode = mode;
        self.display_outdated = true;
        let layers = mode.mode().layers();
        if layers != self.requested {
            self.requested = layers;
            if let Some(control) = &mut self.control {
                control.request_layers(layers);
            }
        }
        if let Some(remember) = &mut self.remember_render_mode {
            remember(
                mode,
                &modes::with_render_mode(self.parameters.running(), mode),
            );
        }
    }

    /// Rebuilds what is drawn for the latest surface and the active mode, if either changed.
    fn refresh_display(&mut self) {
        if !std::mem::take(&mut self.display_outdated) {
            return;
        }
        let (Some(surface), Some(picking)) = (&self.surface, &self.picking) else {
            self.display = self.surface.clone();
            self.overlay = None;
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
            fields: &self.fields,
            arrow_cells,
            arrow_spacing,
        };
        let mode = self.mode.mode();
        self.displays += 1;
        let vertices = mode
            .colors(&data)
            .and_then(|colors| recolored(&surface.vertices, &colors))
            .map_or_else(|| surface.vertices.clone(), Arc::new);
        let overlay = mode
            .arrows(&data)
            .map(|(samples, style)| arrows::triangles(&arrows::arrows(&samples, &style), &style));
        self.display = Some(Surface {
            vertices_id: self.displays,
            vertices,
            ..surface.clone()
        });
        self.overlay = overlay.map(|vertices: Vec<OverlayVertex>| Overlay {
            id: self.displays,
            vertices: Arc::new(vertices),
        });
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

    /// Takes a new surface state. The first one of a simulation brings its triangles.
    fn receive(&mut self, update: SurfaceUpdate) {
        self.surfaces += 1;
        let faces = match update.faces {
            Some(faces) => {
                self.meshes += 1;
                faces
            }
            None => match &self.surface {
                Some(surface) => surface.faces.clone(),
                None => {
                    tracing::error!("Surface update without a mesh");
                    return;
                }
            },
        };
        self.time = Some(update.time);
        self.pace.record(update.step_seconds);
        let surface = Surface {
            mesh_id: self.meshes,
            faces,
            vertices_id: self.surfaces,
            vertices: update.vertices,
        };
        self.update_picking(&surface, update.cells);
        self.surface = Some(surface);
        self.fields = update.fields;
        self.display_outdated = true;
    }

    /// Updates the picking data for a new surface: the mesh when it changed, the radii and
    /// attributes always.
    fn update_picking(&mut self, surface: &Surface, cells: Arc<Vec<CellInfo>>) {
        let vertices = &surface.vertices;
        if cells.len() != vertices.len() {
            tracing::error!(
                cells = cells.len(),
                vertices = vertices.len(),
                "Cell attributes of the wrong size"
            );
            self.picking = None;
            return;
        }
        let max_radius = vertices
            .iter()
            .map(|vertex| f64::from(Vec3::from(vertex.position).length()))
            .fold(1.0, f64::max);
        match &mut self.picking {
            Some(picking) if picking.mesh_id == surface.mesh_id => {
                picking.cells = cells;
                picking.max_radius = max_radius;
            }
            _ => {
                let directions = vertices
                    .iter()
                    .map(|vertex| {
                        Vec3::from(vertex.position)
                            .as_dvec3()
                            .normalize_or(DVec3::X)
                    })
                    .collect();
                // 10 · 4^s + 2 vertices for s subdivisions.
                let subdivisions = (vertices.len().saturating_sub(2) / 10).max(1).ilog2() / 2;
                let mesh = SphereMesh::from_faces(directions, surface.faces.to_vec(), subdivisions);
                // The selected place stays selected on the new mesh (another resolution).
                self.selected = match (self.selected, &self.picking) {
                    (Some(cell), Some(old)) if (cell as usize) < old.mesh.len() => {
                        Some(mesh.nearest(old.mesh.vertex(cell)))
                    }
                    _ => None,
                };
                self.hovered = None;
                self.picking = Some(Picking {
                    mesh_id: surface.mesh_id,
                    mesh,
                    cells,
                    max_radius,
                });
            }
        }
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

    /// The uniforms for a view of `viewport` physical pixels.
    fn uniforms(&self, viewport: egui::Vec2) -> Uniforms {
        let aspect_ratio = viewport.x / viewport.y.max(1.0);
        let eye = self.camera_position();
        let view = glam::camera::rh::view::look_at_mat4(eye, Vec3::ZERO, Vec3::Z);
        // wgpu uses normalised device coordinates with Z in [0, 1] and Y up.
        let field_of_view = vertical_field_of_view(aspect_ratio);
        let projection =
            glam::camera::rh::proj::directx::perspective(field_of_view, aspect_ratio, 0.1, 10.0);
        // Light from the upper left of the camera, so the visible side is always lit.
        let right = Vec3::Z.cross(eye).normalize_or(Vec3::X);
        let light = (eye.normalize() + Vec3::Z * 0.5 - right * 0.6).normalize();
        Uniforms {
            view_projection: (projection * view).to_cols_array_2d(),
            light_direction: light.extend(0.0).to_array(),
            viewport: [viewport.x, viewport.y, 0.0, 0.0],
        }
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
        let mut updated = false;
        while let Ok(event) = self.events.try_recv() {
            match event {
                SimulationEvent::Surface(update) => self.receive(update),
                SimulationEvent::Generating => self.generating = true,
                SimulationEvent::Failed(message) => self.failure = Some(message),
            }
            updated = true;
        }
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
                let uniforms = self.uniforms(rect.size() * pixels_per_point);
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
                ui.painter()
                    .add(eframe::egui_wgpu::Callback::new_paint_callback(
                        rect,
                        PlanetCallback {
                            uniforms,
                            surface: self.display.clone(),
                            overlay: self.overlay.clone(),
                            lines,
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

        let shown = shown.and_then(|(cell, selected)| {
            let picking = self.picking.as_ref()?;
            Some(ShownCell {
                cell,
                info: picking.cells.get(cell as usize)?,
                direction: picking.mesh.vertex(cell),
                selected,
            })
        });
        let cell_panel = self.cell_panel.show(ui.ctx(), shown, self.touch);

        let mut mode = self.mode;
        let view = SwitcherView {
            arrow_spacing: self
                .arrow_cells
                .as_ref()
                .map_or(0.14, |(_, _, spacing)| *spacing),
            points_per_radius: self.points_per_radius,
            waiting: self.waiting_for_fields(),
        };
        let (changed, switcher) = self.switcher.show(ui.ctx(), &mut mode, &view);
        if changed && mode != self.mode {
            self.set_mode(mode);
            ui.ctx().request_repaint();
        }

        let status = SimulationStatus {
            time: self.time,
            duration: self.duration,
            failure: self.failure.as_deref(),
            starting: banner::starting_text(restarted, self.generating),
            step_seconds: self.pace.median(),
        };
        let pace = &mut self.pace;
        let mut hint_action = None;
        let hint = pace
            .has_hint(self.time, self.duration)
            .then_some(|ui: &mut egui::Ui| {
                hint_action = pace.ui(ui, self.time, self.duration);
            });
        let mut launch = self
            .parameters
            .show(ui.ctx(), &status, cell_panel, switcher, hint);
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
