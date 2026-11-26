//! GPU resources and the egui paint callback that draws the planet with wgpu, the sun and the
//! stars behind it (`sky.wgsl`), the overlay of the render mode (arrows) and the outline of the cell under the
//! pointer on top of it.

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
    /// Direction towards the light (`w` unused).
    pub light_direction: [f32; 4],
    /// Size of the view in physical pixels (`x`, `y`), for lines of a fixed width; `z` is 1 if
    /// the render mode shows its colours unlit, 0 if lit; `w` is the number of points along a
    /// ray of the realistic mode's atmosphere, 0 without one (see [`super::atmosphere`]).
    pub viewport: [f32; 4],
    /// The inverse of `view_projection`, for the view direction of the sky's pixels.
    pub inverse_view_projection: [[f32; 4]; 4],
    /// Direction towards the sun (`xyz`) and the angular radius of its disc in radians (`w`).
    pub sun_direction: [f32; 4],
    /// Colour of the sun's disc in sRGB (`xyz`) and the share of the disc the planet does not
    /// hide (`w`), which dims the glow.
    pub sun_color: [f32; 4],
    /// Colour of the light as an sRGB factor (`xyz`) and the share of ambient light (`w`).
    pub light_color: [f32; 4],
    /// The sky's exposure: how many magnitudes fainter the stars appear than to a dark-adapted
    /// view (`x`), the size of a pixel in radians (`y`), the exposure's factor on the sun's sRGB
    /// colour (`z`), and whether the sun and the stars are drawn (`w`, 1 or 0). See
    /// [`super::sky::Adaptation`].
    pub stars: [f32; 4],
    /// The colours of the stars in sRGB, see [`super::sky::STAR_TEMPERATURES`].
    pub star_colors: [[f32; 4]; 8],
    /// The camera's position in the world (`xyz`; `w` unused), for the glint on water
    /// (`realistic.wgsl`).
    pub eye: [f32; 4],
}

/// The most plates whose orientations the detail shader knows; the detail of plates beyond
/// (never seen in practice: runs have tens of plates) is anchored to the world frame.
pub const MAX_PLATES: usize = 256;

/// A vertex of the realistic mode's de-indexed mesh (three own vertices per triangle), which
/// carries the layers of its cell for the sub-cell detail (`realistic.wgsl`, built by
/// `modes::realistic::detail_vertices`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct DetailVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// Bare ground (sRGB) and the plant cover (`a`).
    pub bare: [u8; 4],
    /// Full plant cover (sRGB) and the snow or ice cover (`a`).
    pub plants: [u8; 4],
    /// Snow or ice (sRGB) and the water's roughness (GGX α) / 0.5 (`a`).
    pub snow: [u8; 4],
    /// Open water (sRGB) and the ruggedness of the relief (`a`).
    pub water: [u8; 4],
    /// Positive on land, negative on water: the coast lies where it crosses zero.
    pub coast: f32,
    /// The plates of the triangle's three corners, and the corner of this vertex (0 to 2).
    pub plates: [u16; 4],
    /// For each of the triangle's three corners: the strike of a mountain belt (snorm8 x, y, z
    /// in the world frame), the squeeze of the relief noise along it (bits 24–30) and whether
    /// the cell is ocean (bit 31; its ice is anchored to the world, not to the plate).
    pub ridges: [u32; 3],
    /// The clouds of the cell: cover, type (convective 1, sheets 0), typical optical depth
    /// / 64 and cloud-top height / 16 km (all zero with the clouds off; see
    /// `modes::realistic::cloud_layer`).
    pub clouds: [u8; 4],
    /// The cell's wind for the clouds' drift (snorm8 x, y, z in the world frame, divided by
    /// `modes::realistic::DRIFT_WIND_SCALE`; zero without the drift, #187).
    pub wind: [i8; 4],
}

/// The realistic mode's second uniform buffer: the orientation of every plate (a quaternion
/// from its local frame into the world) and the settings of the detail.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DetailUniforms {
    pub orientations: [[f32; 4]; MAX_PLATES],
    /// The frequency of the first octave in cycles per planet radius (`x`, one per cell
    /// spacing), the most octaves (`y`), the strength of the bump relief (`z`, 0 for none) and
    /// whether mountains use ridged noise (`w`, 0 or 1).
    pub detail: [f32; 4],
    /// The clouds: the most octaves of their pattern after the first (`x`, 0 without clouds),
    /// the frequency of that octave in cycles per planet radius (`y`), whether the cloud tops have
    /// relief (`z`, 0 or 1) and whether the clouds cast shadows (`w`).
    pub clouds: [f32; 4],
    /// The clouds' drift at the moment drawn (zero without it; see
    /// `modes::realistic::cloud_drift`): the seed and weight of the two layers of the pressure
    /// field and of the finer octaves, and their shifts along the wind.
    pub drift: CloudDrift,
}

/// The clouds' drift at one moment, see [`DetailUniforms::drift`].
pub type CloudDrift = [[f32; 4]; 5];

/// The realistic mode's de-indexed mesh and its uniforms; uploaded when its id changes.
#[derive(Clone)]
pub struct Detail {
    pub id: u64,
    pub vertices: Arc<Vec<DetailVertex>>,
    pub uniforms: Arc<DetailUniforms>,
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
    /// The realistic mode's surface with sub-cell detail (`realistic.wgsl`), without clouds.
    detail_pipeline: wgpu::RenderPipeline,
    /// The same with clouds, created when they are first switched on.
    cloud_pipeline: Option<wgpu::RenderPipeline>,
    /// The same with drifting clouds (#187, detail High), created when first shown.
    drift_pipeline: Option<wgpu::RenderPipeline>,
    /// What the realistic pipelines are made with.
    pipeline_layout: wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    msaa_samples: u32,
    /// Whether this frame draws the clouds, and whether they drift.
    clouds_shown: bool,
    drift_shown: bool,
    detail_uniform_buffer: wgpu::Buffer,
    /// The de-indexed mesh last uploaded: its buffer, [`Detail::id`] and number of vertices.
    detail: Option<(wgpu::Buffer, u64, u32)>,
    /// Whether this frame draws the detail mesh instead of the planet mesh.
    detail_shown: bool,
    /// Draws the sun and the stars behind the planet (`sky.wgsl`).
    sky_pipeline: wgpu::RenderPipeline,
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
        entries: &[0, 1].map(|binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }),
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
    let sky_pipeline = sky_pipeline(
        device,
        &pipeline_layout,
        render_state.target_format,
        msaa_samples,
    );
    let detail_pipeline = detail_pipeline(
        device,
        &pipeline_layout,
        render_state.target_format,
        msaa_samples,
        &realistic_shader(false, false),
    );
    let detail_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("detail uniforms"),
        size: std::mem::size_of::<DetailUniforms>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
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
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: detail_uniform_buffer.as_entire_binding(),
            },
        ],
    });
    render_state
        .renderer
        .write()
        .callback_resources
        .insert(PlanetResources {
            pipeline,
            detail_pipeline,
            cloud_pipeline: None,
            drift_pipeline: None,
            pipeline_layout,
            target_format: render_state.target_format,
            msaa_samples,
            clouds_shown: false,
            drift_shown: false,
            detail_uniform_buffer,
            detail: None,
            detail_shown: false,
            sky_pipeline,
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

/// The pipeline of the realistic surface: the de-indexed [`DetailVertex`] mesh, drawn like the
/// planet mesh (depth write, back faces culled).
fn detail_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    msaa_samples: u32,
    source: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("realistic.wgsl"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("realistic surface"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<DetailVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4, 3 => Unorm8x4,
                    4 => Unorm8x4, 5 => Unorm8x4, 6 => Float32, 7 => Uint16x4,
                    8 => Uint32x3, 9 => Unorm8x4, 10 => Snorm8x4
                ],
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
            targets: &[Some(target_format.into())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// The pipeline of the overlay: [`OverlayVertex`] triangles, depth-tested against the planet
/// (mountains in front hide them) without writing depth, so that they are drawn in their order
/// (borders first, see `overlay::OverlayBuilder`).
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

/// The shaders of the realistic mode's surface and of the sky, with the atmosphere
/// (`atmosphere.wgsl`, see [`super::atmosphere`]) they share (WGSL has no includes).
const REALISTIC_SHADER: &str = concat!(
    include_str!("atmosphere.wgsl"),
    include_str!("realistic.wgsl"),
    include_str!("clouds.wgsl")
);
/// The realistic shader with or without the clouds' code (see `CLOUDS_COMPILED` there), and
/// with or without their drift (`CLOUD_DRIFT_COMPILED` in `clouds.wgsl`).
fn realistic_shader(clouds: bool, drift: bool) -> String {
    let mut source = REALISTIC_SHADER.to_owned();
    if !clouds {
        source = source.replace(
            "const CLOUDS_COMPILED: bool = true;",
            "const CLOUDS_COMPILED: bool = false;",
        );
    }
    if !(clouds && drift) {
        source = source.replace(
            "const CLOUD_DRIFT_COMPILED: bool = true;",
            "const CLOUD_DRIFT_COMPILED: bool = false;",
        );
    }
    source
}
const SKY_SHADER: &str = concat!(include_str!("atmosphere.wgsl"), include_str!("sky.wgsl"));

/// The pipeline of the sky (the sun and the stars): one full-screen triangle behind the planet, depth-tested
/// against it without writing depth, added to the background.
fn sky_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    msaa_samples: u32,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sky.wgsl"),
        source: wgpu::ShaderSource::Wgsl(SKY_SHADER.into()),
    });
    let additive = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        // The target's alpha stays as it is.
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sky"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
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
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(additive),
                write_mask: wgpu::ColorWrites::ALL,
            })],
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
    /// The realistic mode's surface with sub-cell detail, drawn instead of `vertices`.
    pub detail: Option<Detail>,
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
    /// Whether to draw the sky pass: the sun and the stars (only when the planet is lit by the
    /// sun) and the realistic mode's atmosphere off the disc.
    pub sky: bool,
    /// The clouds' drift at this moment, while it is shown (realistic mode, clouds on, detail
    /// High; #187).
    pub cloud_drift: Option<CloudDrift>,
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
        resources.detail_shown = false;
        resources.clouds_shown = false;
        resources.drift_shown = false;
        if let Some(detail) = self
            .surface
            .as_ref()
            .and_then(|surface| surface.detail.as_ref())
            .filter(|detail| !detail.vertices.is_empty())
        {
            let drift = self.cloud_drift.filter(|_| detail.uniforms.clouds[0] > 0.0);
            let mut uniforms = *detail.uniforms;
            uniforms.drift = drift.unwrap_or_default();
            queue.write_buffer(
                &resources.detail_uniform_buffer,
                0,
                bytemuck::bytes_of(&uniforms),
            );
            let bytes: &[u8] = bytemuck::cast_slice(detail.vertices.as_slice());
            let fits = |buffer: &wgpu::Buffer| buffer.size() >= bytes.len() as u64;
            match &mut resources.detail {
                Some((_, id, _)) if *id == detail.id => {}
                Some((buffer, id, count)) if fits(buffer) => {
                    queue.write_buffer(buffer, 0, bytes);
                    *id = detail.id;
                    *count = detail.vertices.len() as u32;
                }
                _ => {
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("realistic surface"),
                        contents: bytes,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    });
                    resources.detail = Some((buffer, detail.id, detail.vertices.len() as u32));
                }
            }
            resources.detail_shown = true;
            if detail.uniforms.clouds[0] > 0.0 {
                let pipeline = if drift.is_some() {
                    &mut resources.drift_pipeline
                } else {
                    &mut resources.cloud_pipeline
                };
                if pipeline.is_none() {
                    *pipeline = Some(detail_pipeline(
                        device,
                        &resources.pipeline_layout,
                        resources.target_format,
                        resources.msaa_samples,
                        &realistic_shader(true, drift.is_some()),
                    ));
                }
                resources.clouds_shown = true;
                resources.drift_shown = drift.is_some();
            }
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
        render_pass.set_bind_group(0, &resources.bind_group, &[]);
        let Some(mesh) = resources.mesh.as_ref().filter(|mesh| mesh.has_vertices) else {
            // No planet yet: only the sun.
            if self.sky {
                render_pass.set_pipeline(&resources.sky_pipeline);
                render_pass.draw(0..3, 0..1);
            }
            return;
        };
        match &resources.detail {
            Some((buffer, _, count)) if resources.detail_shown => {
                let clouds = if resources.drift_shown {
                    &resources.drift_pipeline
                } else {
                    &resources.cloud_pipeline
                };
                let pipeline = match clouds {
                    Some(pipeline) if resources.clouds_shown => pipeline,
                    _ => &resources.detail_pipeline,
                };
                render_pass.set_pipeline(pipeline);
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..*count, 0..1);
            }
            _ => {
                render_pass.set_pipeline(&resources.pipeline);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        // After the planet, so that its depth hides the sun where the planet is in front.
        if self.sky {
            render_pass.set_pipeline(&resources.sky_pipeline);
            render_pass.draw(0..3, 0..1);
        }
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

#[cfg(test)]
mod tests {
    /// Every shader of the view parses, validates (without optional capabilities) and
    /// translates to GLSL ES 3.00 for WebGL2 with naga, wgpu's shader translator: a shader that
    /// naga cannot translate for the web fails here, not only in the browser. (wgpu's own GL
    /// options and the device limits of WebGL2 are not checked.)
    #[test]
    fn shaders_translate_to_webgl2() {
        use naga::back::glsl;
        let without_clouds = super::realistic_shader(false, false);
        let static_clouds = super::realistic_shader(true, false);
        let drifting_clouds = super::realistic_shader(true, true);
        assert_ne!(without_clouds, static_clouds);
        assert_ne!(static_clouds, drifting_clouds);
        assert_eq!(drifting_clouds, super::REALISTIC_SHADER);
        // No drift without the clouds' code.
        assert_eq!(super::realistic_shader(false, true), without_clouds);
        let shaders = [
            ("planet.wgsl", include_str!("planet.wgsl")),
            ("sky.wgsl", super::SKY_SHADER),
            (
                "realistic.wgsl with drifting clouds",
                drifting_clouds.as_str(),
            ),
            ("realistic.wgsl with static clouds", static_clouds.as_str()),
            ("realistic.wgsl without clouds", without_clouds.as_str()),
            ("overlay.wgsl", include_str!("overlay.wgsl")),
            ("outline.wgsl", include_str!("outline.wgsl")),
        ];
        for (name, source) in shaders {
            let module = naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|error| panic!("{name}: {}", error.emit_to_string(source)));
            let info = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            for entry_point in &module.entry_points {
                let options = glsl::Options {
                    version: glsl::Version::Embedded {
                        version: 300,
                        is_webgl: true,
                    },
                    ..glsl::Options::default()
                };
                let pipeline_options = glsl::PipelineOptions {
                    shader_stage: entry_point.stage,
                    entry_point: entry_point.name.clone(),
                    multiview: None,
                };
                let mut output = String::new();
                glsl::Writer::new(
                    &mut output,
                    &module,
                    &info,
                    &options,
                    &pipeline_options,
                    naga::proc::BoundsCheckPolicies::default(),
                )
                .and_then(|mut writer| writer.write().map(|_| ()))
                .unwrap_or_else(|error| panic!("{name} {}: {error:?}", entry_point.name));
                assert!(output.contains("#version 300 es"), "{name}");
                // WebGL2 guarantees 15 vectors between the stages
                // (MAX_VARYING_VECTORS, one location each here).
                if entry_point.stage == naga::ShaderStage::Vertex {
                    let varyings = output
                        .lines()
                        .filter(|line| line.contains(" out ") && line.contains("_vs2fs_location"))
                        .count();
                    assert!(varyings <= 15, "{name}: {varyings} varyings");
                    if name.starts_with("realistic.wgsl") {
                        assert_eq!(varyings, 11, "{output}");
                    }
                }
            }
        }
    }
}
