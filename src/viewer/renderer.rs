//! GPU resources and the egui paint callback that draws the planet with wgpu, the overlay of
//! the render mode (arrows) and the outline of the cell under the pointer on top of it.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};
use wgpu::util::DeviceExt;

use super::arrows::OverlayVertex;

/// Format of the depth buffer requested from eframe; must match [`super::DEPTH_BUFFER_BITS`].
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A vertex of the planet surface mesh.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// sRGB colour with alpha.
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Uniforms {
    pub view_projection: [[f32; 4]; 4],
    pub light_direction: [f32; 4],
    /// Size of the view in physical pixels (`x`, `y`), for lines of a fixed width; `z` is 1 if
    /// the render mode shows its colours unlit, 0 if lit (`w` unused).
    pub viewport: [f32; 4],
}

/// A straight piece of a line drawn over the planet with a constant width in pixels (see
/// `outline.wgsl`). It is depth-tested against the planet, but does not write depth.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LineSegment {
    pub start: [f32; 3],
    /// Width in physical pixels.
    pub width: f32,
    pub end: [f32; 3],
    pub _padding: f32,
    /// Colour with alpha (opaque colours only; the line is not blended).
    pub color: [f32; 4],
}

/// The most line segments drawn in one frame.
pub const MAX_LINE_SEGMENTS: usize = 64;

/// GPU resources, stored in egui's callback resources.
struct PlanetResources {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    line_pipeline: wgpu::RenderPipeline,
    /// Room for [`MAX_LINE_SEGMENTS`] segments.
    line_buffer: wgpu::Buffer,
    /// The number of segments in `line_buffer` for this frame.
    line_count: u32,
    /// The mesh of the running simulation, once its first surface arrived.
    mesh: Option<MeshBuffers>,
    overlay_pipeline: wgpu::RenderPipeline,
    /// The overlay last uploaded: its buffer, [`Overlay::id`] and number of vertices.
    overlay: Option<(wgpu::Buffer, u64, u32)>,
    /// Whether this frame has an overlay.
    overlay_shown: bool,
}

/// Vertex and index buffers of the planet mesh. They are replaced when a simulation with
/// another mesh starts.
struct MeshBuffers {
    /// [`Surface::mesh_id`] of the mesh.
    id: u64,
    /// [`Surface::vertices_id`] of the vertices last written, if any.
    vertices_id: Option<u64>,
    /// Whether the vertex buffer holds vertices.
    has_vertices: bool,
    vertex_buffer: wgpu::Buffer,
    vertex_count: usize,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl MeshBuffers {
    fn new(device: &wgpu::Device, id: u64, vertex_count: usize, faces: &[[u32; 3]]) -> Self {
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("planet vertices"),
            size: (vertex_count * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("planet indices"),
            contents: bytemuck::cast_slice(faces),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            id,
            vertices_id: None,
            has_vertices: false,
            vertex_buffer,
            vertex_count,
            index_buffer,
            index_count: (faces.len() * 3) as u32,
        }
    }
}

/// Creates the GPU pipeline; the mesh buffers follow with the first surface of a simulation.
pub fn initialize(render_state: &egui_wgpu::RenderState, msaa_samples: u32) {
    let device = &render_state.device;
    let shader = device.create_shader_module(wgpu::include_wgsl!("planet.wgsl"));

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("planet uniforms"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("planet"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("planet"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: msaa_samples,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(render_state.target_format.into())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview_mask: None,
        cache: None,
    });

    let line_pipeline = line_pipeline(
        device,
        &pipeline_layout,
        render_state.target_format,
        msaa_samples,
    );
    let line_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("planet lines"),
        size: (MAX_LINE_SEGMENTS * std::mem::size_of::<LineSegment>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let overlay_pipeline = overlay_pipeline(
        device,
        &pipeline_layout,
        render_state.target_format,
        msaa_samples,
    );

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("planet uniforms"),
        size: std::mem::size_of::<Uniforms>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("planet uniforms"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(PlanetResources {
            pipeline,
            uniform_buffer,
            bind_group,
            line_pipeline,
            line_buffer,
            line_count: 0,
            mesh: None,
            overlay_pipeline,
            overlay: None,
            overlay_shown: false,
        });
}

/// The pipeline of the overlay: [`OverlayVertex`] triangles, depth-tested against the planet
/// (mountains in front hide them) without writing depth, so that they are drawn in their order
/// (borders first, see `arrows::triangles`).
fn overlay_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    msaa_samples: u32,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("overlay.wgsl"));
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("planet overlay"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<OverlayVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: msaa_samples,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(target_format.into())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// The pipeline of [`LineSegment`]s: one instance per segment, drawn as a quad of six vertices
/// that the vertex shader widens in screen space.
fn line_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    msaa_samples: u32,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("outline.wgsl"));
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("planet lines"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<LineSegment>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x3, 1 => Float32, 2 => Float32x3, 3 => Float32, 4 => Float32x4
                ],
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        // Hidden behind the planet (mountains in front of the cell), but drawn over each other
        // in order: the dark border first, the light line on top.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: msaa_samples,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(target_format.into())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// The latest surface to draw. It is passed with every frame, and the renderer uploads only
/// what changed, so a frame that egui discards (e.g. a sizing pass) loses nothing.
#[derive(Clone)]
pub struct Surface {
    /// Changes with every new mesh (a new simulation).
    pub mesh_id: u64,
    pub faces: Arc<Vec<[u32; 3]>>,
    /// Changes with every new state of the surface.
    pub vertices_id: u64,
    pub vertices: Arc<Vec<Vertex>>,
}

/// Triangles drawn over the planet by the render mode (arrows). Like [`Surface`], it is passed
/// with every frame and uploaded only when its id changes.
#[derive(Clone)]
pub struct Overlay {
    pub id: u64,
    pub vertices: Arc<Vec<OverlayVertex>>,
}

/// Draws the planet into the area of an egui widget.
pub struct PlanetCallback {
    pub uniforms: Uniforms,
    pub surface: Option<Surface>,
    pub overlay: Option<Overlay>,
    /// Lines to draw over the planet, at most [`MAX_LINE_SEGMENTS`] (the rest are dropped).
    pub lines: Vec<LineSegment>,
}

impl egui_wgpu::CallbackTrait for PlanetCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources: &mut PlanetResources = callback_resources
            .get_mut()
            .expect("planet resources are initialised at startup");
        queue.write_buffer(
            &resources.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.uniforms),
        );
        let lines = &self.lines[..self.lines.len().min(MAX_LINE_SEGMENTS)];
        if !lines.is_empty() {
            queue.write_buffer(&resources.line_buffer, 0, bytemuck::cast_slice(lines));
        }
        resources.line_count = lines.len() as u32;
        resources.overlay_shown = false;
        if let Some(overlay) = self.overlay.as_ref().filter(|o| !o.vertices.is_empty()) {
            let bytes: &[u8] = bytemuck::cast_slice(overlay.vertices.as_slice());
            let fits = |buffer: &wgpu::Buffer| buffer.size() >= bytes.len() as u64;
            match &mut resources.overlay {
                Some((_, id, _)) if *id == overlay.id => {}
                Some((buffer, id, count)) if fits(buffer) => {
                    queue.write_buffer(buffer, 0, bytes);
                    *id = overlay.id;
                    *count = overlay.vertices.len() as u32;
                }
                _ => {
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("planet overlay"),
                        contents: bytes,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    });
                    resources.overlay = Some((buffer, overlay.id, overlay.vertices.len() as u32));
                }
            }
            resources.overlay_shown = true;
        }
        let Some(surface) = &self.surface else {
            return Vec::new();
        };
        if resources.mesh.as_ref().map(|mesh| mesh.id) != Some(surface.mesh_id) {
            resources.mesh = Some(MeshBuffers::new(
                device,
                surface.mesh_id,
                surface.vertices.len(),
                &surface.faces,
            ));
        }
        if let Some(mesh) = &mut resources.mesh
            && mesh.vertices_id != Some(surface.vertices_id)
        {
            mesh.vertices_id = Some(surface.vertices_id);
            // Vertices of another mesh (which should not happen) are not drawn.
            if surface.vertices.len() == mesh.vertex_count {
                let vertices: &[Vertex] = &surface.vertices;
                queue.write_buffer(&mesh.vertex_buffer, 0, bytemuck::cast_slice(vertices));
                mesh.has_vertices = true;
            } else {
                tracing::error!(
                    vertices = surface.vertices.len(),
                    expected = mesh.vertex_count,
                    "Surface update of the wrong size"
                );
            }
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let resources: &PlanetResources = callback_resources
            .get()
            .expect("planet resources are initialised at startup");
        let Some(mesh) = resources.mesh.as_ref().filter(|mesh| mesh.has_vertices) else {
            return;
        };
        render_pass.set_pipeline(&resources.pipeline);
        render_pass.set_bind_group(0, &resources.bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        if resources.overlay_shown
            && let Some((buffer, _, count)) = &resources.overlay
        {
            render_pass.set_pipeline(&resources.overlay_pipeline);
            render_pass.set_vertex_buffer(0, buffer.slice(..));
            render_pass.draw(0..*count, 0..1);
        }
        if resources.line_count > 0 {
            render_pass.set_pipeline(&resources.line_pipeline);
            render_pass.set_vertex_buffer(0, resources.line_buffer.slice(..));
            render_pass.draw(0..6, 0..resources.line_count);
        }
    }
}
