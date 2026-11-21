//! Interactive 3D view of the planet.
//!
//! The window is an [`eframe`] (egui) application, which runs on Windows, macOS and Linux, and
//! lets later user interface elements be drawn on top of the 3D view. The planet itself is
//! drawn with wgpu through an egui paint callback (see [`renderer`]).
//!
//! The simulation runs on a separate thread (in a Web Worker on the web, see `crate::web`) and
//! sends new surface meshes to the view through a channel. The parameter panel
//! ([`parameters`]) restarts it with other options.

mod parameters;
mod renderer;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use eframe::egui;
use glam::Vec3;

use crate::Cli;
use crate::planet::Planet;
use crate::render;
use parameters::{ParameterPanel, SimulationStatus};
#[cfg(target_arch = "wasm32")]
pub use renderer::Vertex;
#[cfg(not(target_arch = "wasm32"))]
use renderer::Vertex;
use renderer::{PlanetCallback, Surface, Uniforms};

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

/// A new state of the planet surface to display.
pub struct SurfaceUpdate {
    /// Simulated time in Myr.
    pub time: f64,
    pub vertices: Arc<Vec<Vertex>>,
    /// The triangles of the mesh: sent with the first update of a simulation, `None` after.
    pub faces: Option<Arc<Vec<[u32; 3]>>>,
}

/// A message from the simulation to the view.
pub enum SimulationEvent {
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
/// through the sender and calls the callback to wake up the view after each. Called once the
/// view exists, and again on every restart from the parameter panel, with a new channel: a
/// previous simulation should stop when sending to its channel fails.
pub type StartSimulation = Box<dyn FnMut(Launch, Sender<SimulationEvent>, Box<dyn Fn() + Send>)>;

/// Creates the viewer app; shared by the desktop window and the web page.
fn app_creator(
    initial: Launch,
    msaa_samples: u16,
    ui_options: UiOptions,
    start_simulation: StartSimulation,
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
            start_simulation,
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
        ),
    )
    .map_err(|error| anyhow::anyhow!("failed to run the 3D view: {error}"))?;
    tracing::info!("3D view closed");
    Ok(())
}

/// Starts the 3D view in `canvas` of the web page. eframe's web painter does not multisample.
#[cfg(target_arch = "wasm32")]
pub async fn run_web(
    canvas: web_sys::HtmlCanvasElement,
    initial: Launch,
    ui_options: UiOptions,
    start_simulation: StartSimulation,
) -> anyhow::Result<()> {
    let options = eframe::WebOptions {
        depth_buffer: DEPTH_BUFFER_BITS,
        ..Default::default()
    };
    eframe::WebRunner::new()
        .start(
            canvas,
            options,
            app_creator(initial, 1, ui_options, start_simulation),
        )
        .await
        .map_err(|error| anyhow::anyhow!("failed to start the 3D view: {error:?}"))
}

struct Viewer {
    events: Receiver<SimulationEvent>,
    start_simulation: StartSimulation,
    parameters: ParameterPanel,
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
    /// The latest surface; the previous simulation's until the new one sends its first.
    surface: Option<Surface>,
    /// The number of meshes and surface states received, for [`Surface`]'s ids.
    meshes: u64,
    surfaces: u64,
}

impl Viewer {
    fn new(ctx: &egui::Context, initial: Launch, start_simulation: StartSimulation) -> Self {
        let parameters = ParameterPanel::new(&initial.args);
        let (_, events) = std::sync::mpsc::channel();
        let mut viewer = Self {
            events,
            start_simulation,
            parameters,
            yaw: 0.0,
            pitch: 0.4,
            time: None,
            duration: initial.cli.duration,
            failure: None,
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
    fn start(&mut self, ctx: &egui::Context, launch: Launch) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.events = receiver;
        self.time = None;
        self.duration = launch.cli.duration;
        self.failure = None;
        let ctx = ctx.clone();
        (self.start_simulation)(launch, sender, Box::new(move || ctx.request_repaint()));
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
        self.surface = Some(Surface {
            mesh_id: self.meshes,
            faces,
            vertices_id: self.surfaces,
            vertices: update.vertices,
        });
    }

    fn camera_position(&self) -> Vec3 {
        // The planet's rotation axis is +z.
        Vec3::new(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
        ) * CAMERA_DISTANCE
    }

    fn uniforms(&self, aspect_ratio: f32) -> Uniforms {
        let eye = self.camera_position();
        let view = glam::camera::rh::view::look_at_mat4(eye, Vec3::ZERO, Vec3::Z);
        // wgpu uses normalised device coordinates with Z in [0, 1] and Y up.
        // In portrait (phones), the field of view applies horizontally, so the planet fits.
        let field_of_view = if aspect_ratio < 1.0 {
            2.0 * ((FIELD_OF_VIEW / 2.0).tan() / aspect_ratio.max(0.01)).atan()
        } else {
            FIELD_OF_VIEW
        };
        let projection =
            glam::camera::rh::proj::directx::perspective(field_of_view, aspect_ratio, 0.1, 10.0);
        // Light from the upper left of the camera, so the visible side is always lit.
        let right = Vec3::Z.cross(eye).normalize_or(Vec3::X);
        let light = (eye.normalize() + Vec3::Z * 0.5 - right * 0.6).normalize();
        Uniforms {
            view_projection: (projection * view).to_cols_array_2d(),
            light_direction: light.extend(0.0).to_array(),
        }
    }
}

impl eframe::App for Viewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut updated = false;
        while let Ok(event) = self.events.try_recv() {
            match event {
                SimulationEvent::Surface(update) => self.receive(update),
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

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(5, 5, 15)))
            .show(ui, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), egui::Sense::drag());
                let delta = response.drag_delta();
                self.yaw -= delta.x * ROTATION_SPEED;
                self.pitch = (self.pitch + delta.y * ROTATION_SPEED).clamp(-1.5, 1.5);

                let aspect_ratio = rect.width() / rect.height().max(1.0);
                ui.painter()
                    .add(eframe::egui_wgpu::Callback::new_paint_callback(
                        rect,
                        PlanetCallback {
                            uniforms: self.uniforms(aspect_ratio),
                            surface: self.surface.clone(),
                        },
                    ));
            });

        let status = SimulationStatus {
            time: self.time,
            duration: self.duration,
            failure: self.failure.as_deref(),
        };
        if let Some(launch) = self.parameters.show(ui.ctx(), &status) {
            tracing::info!(args = ?launch.args, "Restarting the simulation");
            self.parameters.set_running(launch.args.clone());
            let ctx = ui.ctx().clone();
            self.start(&ctx, launch);
        }
    }
}
