//! Interactive 3D view of the planet.
//!
//! The window is an [`eframe`] (egui) application, which runs on Windows, macOS and Linux, and
//! lets later user interface elements be drawn on top of the 3D view. The planet itself is
//! drawn with wgpu through an egui paint callback (see [`renderer`]).
//!
//! The simulation runs on a separate thread (in a Web Worker on the web, see `crate::web`) and
//! sends new surface meshes to the view through a channel.

mod renderer;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use eframe::egui;
use glam::Vec3;

use crate::planet::Planet;
use crate::render;
#[cfg(target_arch = "wasm32")]
pub use renderer::Vertex;
#[cfg(not(target_arch = "wasm32"))]
use renderer::Vertex;
use renderer::{PlanetCallback, Uniforms};

/// Bits of the depth buffer requested from eframe; must match [`renderer::DEPTH_FORMAT`].
const DEPTH_BUFFER_BITS: u8 = 32;
/// Multisample anti-aliasing samples (desktop only; the web painter does not multisample).
#[cfg(not(target_arch = "wasm32"))]
const MSAA_SAMPLES: u16 = 4;
/// Vertical field of view of the camera in radians.
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

/// Called once the view exists, with a sender for surface updates and a callback that wakes up the
/// view; it should start the simulation (on another thread, or in a Web Worker on the web).
pub type StartSimulation = Box<dyn FnOnce(Sender<SurfaceUpdate>, Box<dyn Fn() + Send>)>;

/// Creates the viewer app; shared by the desktop window and the web page.
fn app_creator(
    vertex_count: usize,
    faces: Vec<[u32; 3]>,
    msaa_samples: u16,
    start_simulation: StartSimulation,
) -> eframe::AppCreator<'static> {
    Box::new(move |creation_context| {
        let render_state = creation_context
            .wgpu_render_state
            .as_ref()
            .ok_or("the wgpu renderer is not available")?;
        renderer::initialize(render_state, vertex_count, &faces, u32::from(msaa_samples));

        let (sender, receiver) = std::sync::mpsc::channel();
        let context = creation_context.egui_ctx.clone();
        start_simulation(sender, Box::new(move || context.request_repaint()));
        Ok(Box::new(Viewer::new(receiver)))
    })
}

/// Opens the 3D view and blocks until the window is closed.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(
    vertex_count: usize,
    faces: Vec<[u32; 3]>,
    start_simulation: impl FnOnce(Sender<SurfaceUpdate>, Box<dyn Fn() + Send>) + 'static,
) -> anyhow::Result<()> {
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
            vertex_count,
            faces,
            MSAA_SAMPLES,
            Box::new(start_simulation),
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
    vertex_count: usize,
    faces: Vec<[u32; 3]>,
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
            app_creator(vertex_count, faces, 1, start_simulation),
        )
        .await
        .map_err(|error| anyhow::anyhow!("failed to start the 3D view: {error:?}"))
}

struct Viewer {
    updates: Receiver<SurfaceUpdate>,
    /// Rotation of the camera around the planet's axis in radians.
    yaw: f32,
    /// Elevation angle of the camera above the equator in radians.
    pitch: f32,
    /// Simulated time of the displayed state.
    time: Option<f64>,
}

impl Viewer {
    fn new(updates: Receiver<SurfaceUpdate>) -> Self {
        Self {
            updates,
            yaw: 0.0,
            pitch: 0.4,
            time: None,
        }
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
        let projection =
            glam::camera::rh::proj::directx::perspective(FIELD_OF_VIEW, aspect_ratio, 0.1, 10.0);
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
        // Only the latest update matters.
        let mut vertices = None;
        while let Ok(update) = self.updates.try_recv() {
            self.time = Some(update.time);
            vertices = Some(update.vertices);
        }
        if vertices.is_some()
            && let Some(time) = self.time
        {
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
                            vertices,
                        },
                    ));
            });
    }
}
