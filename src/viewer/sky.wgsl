// Draws the sun behind the planet: a full-screen triangle just in front of the far plane,
// depth-tested against the planet (which hides the sun where it is in front), added to the
// background. The disc has the true angular size of the sun (from the sun model), limb
// darkening, and a glow around it as a camera would show it.

struct Uniforms {
    view_projection: mat4x4<f32>,
    light_direction: vec4<f32>,
    viewport: vec4<f32>,
    // Inverse of view_projection: from normalised device coordinates to the world.
    inverse_view_projection: mat4x4<f32>,
    // Unit vector towards the sun (xyz) and the angular radius of its disc in radians (w).
    sun_direction: vec4<f32>,
    // Colour of the sun's disc in sRGB (rgb; w unused).
    sun_color: vec4<f32>,
    light_color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

// Depth of the sky: behind the planet (whose depth stays below 0.995 with the camera at 4.5
// radii and the far plane at 10), in front of the far plane.
const SKY_DEPTH: f32 = 0.99999;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    // One triangle that covers the screen: (-1, -1), (3, -1), (-1, 3).
    let ndc = vec2<f32>(f32((index << 1u) & 2u) * 2.0 - 1.0, f32(index & 2u) * 2.0 - 1.0);
    var output: VertexOutput;
    output.clip_position = vec4<f32>(ndc, SKY_DEPTH, 1.0);
    output.ndc = ndc;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let near = uniforms.inverse_view_projection * vec4<f32>(input.ndc, 0.0, 1.0);
    let far = uniforms.inverse_view_projection * vec4<f32>(input.ndc, 1.0, 1.0);
    let direction = normalize(far.xyz / far.w - near.xyz / near.w);
    let sun = uniforms.sun_direction.xyz;
    let radius = uniforms.sun_direction.w;
    // Angle from the sun's centre; atan2 of sine and cosine stays precise at small angles.
    let angle = atan2(length(cross(direction, sun)), dot(direction, sun));
    // Size of a pixel in radians, for an anti-aliased edge of the disc.
    let pixel = max(fwidth(angle), 1e-6);
    let disc = clamp((radius - angle) / pixel + 0.5, 0.0, 1.0);
    // Limb darkening (linear law, u = 0.6).
    let r = min(angle / radius, 1.0);
    let limb = 1.0 - 0.6 * (1.0 - sqrt(1.0 - r * r));
    // Glow around the disc: a halo falling off with the square of the angle, as bright as
    // the darkened limb at the edge, and a faint wide one (about 3°).
    let outside = max(angle, radius);
    let glow = 0.4 * (radius / outside) * (radius / outside) + 0.06 * exp(-angle / 0.05);
    let intensity = mix(glow, limb, disc);
    return vec4<f32>(uniforms.sun_color.rgb * intensity, 0.0);
}
