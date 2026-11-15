// Renders the planet surface with per-vertex colours and simple diffuse lighting.

struct Uniforms {
    view_projection: mat4x4<f32>,
    // Direction towards the light, in world space (w unused).
    light_direction: vec4<f32>,
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
    let light = 0.3 + 0.7 * diffuse;
    return vec4<f32>(input.color.rgb * light, 1.0);
}
