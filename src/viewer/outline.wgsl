// Draws line segments over the planet with a constant width in pixels: every segment is an
// instance of six vertices (a quad), widened in screen space and extended by half the width at
// both ends, so that the segments of a polygon join without gaps.

struct Uniforms {
    view_projection: mat4x4<f32>,
    light_direction: vec4<f32>,
    // Size of the view in physical pixels (x, y).
    viewport: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct SegmentInput {
    @location(0) start: vec3<f32>,
    // Width in physical pixels.
    @location(1) width: f32,
    @location(2) end: vec3<f32>,
    @location(3) padding: f32,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32, segment: SegmentInput) -> VertexOutput {
    // Two triangles: (start, -), (start, +), (end, -) and (end, -), (start, +), (end, +).
    var ends = array<f32, 6>(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);
    var sides = array<f32, 6>(-1.0, 1.0, -1.0, -1.0, 1.0, 1.0);
    let at_end = ends[index];
    let side = sides[index];

    let start = uniforms.view_projection * vec4<f32>(segment.start, 1.0);
    let end = uniforms.view_projection * vec4<f32>(segment.end, 1.0);
    let half_viewport = uniforms.viewport.xy * 0.5;
    var along = end.xy / end.w * half_viewport - start.xy / start.w * half_viewport;
    if length(along) < 1e-6 {
        along = vec2<f32>(1.0, 0.0);
    }
    along = normalize(along);
    let across = vec2<f32>(-along.y, along.x);
    let offset = (across * side + along * (at_end * 2.0 - 1.0)) * segment.width * 0.5;

    let clip = select(start, end, at_end > 0.5);
    var output: VertexOutput;
    output.clip_position = vec4<f32>(clip.xy + offset / half_viewport * clip.w, clip.zw);
    output.color = segment.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
