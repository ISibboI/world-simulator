//! GPU resources and the egui paint callback that draws the planet with wgpu.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};
use wgpu::util::DeviceExt;

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
}

/// GPU resources, stored in egui's callback resources.
struct PlanetResources {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The mesh of the running simulation, once its first surface arrived.
    mesh: Option<MeshBuffers>,
}

/// Vertex and index buffers of the planet mesh. They are replaced when a simulation with
/// another mesh starts.
struct MeshBuffers {
    vertex_buffer: wgpu::Buffer,
    vertex_count: usize,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl MeshBuffers {
    fn new(device: &wgpu::Device, vertex_count: usize, faces: &[[u32; 3]]) -> Self {
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
            mesh: None,
        });
}

/// Draws the planet into the area of an egui widget.
pub struct PlanetCallback {
    pub uniforms: Uniforms,
    /// New vertex data to upload before drawing, if the planet changed.
    pub vertices: Option<Arc<Vec<Vertex>>>,
    /// The triangles of a new mesh, with the first vertices of a new simulation.
    pub faces: Option<Arc<Vec<[u32; 3]>>>,
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
        if let (Some(faces), Some(vertices)) = (&self.faces, &self.vertices) {
            resources.mesh = Some(MeshBuffers::new(device, vertices.len(), faces));
        }
        if let (Some(vertices), Some(mesh)) = (&self.vertices, &resources.mesh) {
            // Vertices of another mesh (which should not happen) are not drawn.
            if vertices.len() == mesh.vertex_count {
                queue.write_buffer(&mesh.vertex_buffer, 0, bytemuck::cast_slice(vertices));
            } else {
                tracing::error!(
                    vertices = vertices.len(),
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
        let Some(mesh) = &resources.mesh else {
            return;
        };
        render_pass.set_pipeline(&resources.pipeline);
        render_pass.set_bind_group(0, &resources.bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
    }
}
