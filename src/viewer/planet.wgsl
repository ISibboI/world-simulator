// Renders the planet surface with per-vertex colours and simple diffuse lighting: from the
// camera or the sun, with a share of ambient light (see `lighting.rs`). The realistic mode draws
// its own surface (`realistic.wgsl`).

struct Uniforms {
    view_projection: mat4x4<f32>,
    // Direction towards the light, in world space (w unused).
    light_direction: vec4<f32>,
    // Size of the view in physical pixels (x, y; used by outline.wgsl), and 1 in z if the
    // render mode shows its colours unlit (0: lit).
    viewport: vec4<f32>,
    // Used by sky.wgsl.
    inverse_view_projection: mat4x4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    // Colour of the light as a factor on the sRGB colours (rgb), share of ambient light (w).
    light_color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = uniforms.view_projection * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let diffuse = max(dot(normalize(input.normal), uniforms.light_direction.xyz), 0.0);
    let ambient = uniforms.light_color.w;
    let lit = vec3<f32>(ambient) + (1.0 - ambient) * diffuse * uniforms.light_color.rgb;
    let light = mix(lit, vec3<f32>(1.0), uniforms.viewport.z);
    return vec4<f32>(min(input.color.rgb * light, vec3<f32>(1.0)), 1.0);
}
