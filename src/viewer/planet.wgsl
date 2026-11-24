// Renders the planet surface with per-vertex colours and simple diffuse lighting: from the
// camera or the sun, with a share of ambient light (see `lighting.rs`). The realistic mode's
// colours are materials (see `modes/realistic.rs`): reflectances lit in linear light, and water
// (alpha < 0.5, its roughness) with a sun glint.

struct Uniforms {
    view_projection: mat4x4<f32>,
    // Direction towards the light, in world space (w unused).
    light_direction: vec4<f32>,
    // Size of the view in physical pixels (x, y; used by outline.wgsl), 1 in z if the render
    // mode shows its colours unlit (0: lit), and 1 in w for the realistic mode's materials.
    viewport: vec4<f32>,
    // Used by sky.wgsl.
    inverse_view_projection: mat4x4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    // Colour of the light as a factor on the sRGB colours (rgb), share of ambient light (w).
    light_color: vec4<f32>,
    // Used by sky.wgsl.
    stars: vec4<f32>,
    star_colors: array<vec4<f32>, 8>,
    // The camera's position (xyz).
    eye: vec4<f32>,
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
    @location(2) position: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = uniforms.view_projection * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    output.position = input.position;
    return output;
}

const PI: f32 = 3.14159265;
// Reflectance of water at normal incidence (refractive index 1.33).
const WATER_F0: f32 = 0.02;
// Colour of the haze at the limb (Rayleigh scattering of a thin atmosphere), in linear light.
const HAZE: vec3<f32> = vec3<f32>(0.11, 0.2, 0.42);

// The realistic mode: the vertex colour is a reflectance (sRGB-encoded), its alpha the
// roughness of water (< 0.5) or 1 for land. Lit in linear light, encoded to sRGB at the end.
fn realistic(input: VertexOutput, mesh_normal: vec3<f32>) -> vec3<f32> {
    let l = uniforms.light_direction.xyz;
    let v = normalize(uniforms.eye.xyz - input.position);
    let up = normalize(input.position);
    let albedo = pow(max(input.color.rgb, vec3<f32>(0.0)), vec3<f32>(2.2));
    // The light's colour carries the exposure's gain as a factor on sRGB values.
    let sun = pow(max(uniforms.light_color.rgb, vec3<f32>(0.0)), vec3<f32>(2.2));
    let ambient = pow(uniforms.light_color.w, 2.2);
    // Water (alpha below 0.5, blended over half a coast triangle) lies flat on the sphere.
    let water = 1.0 - smoothstep(0.4, 0.6, input.color.a);
    let n = normalize(mix(mesh_normal, up, water));
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 1e-3);
    // Sun glint: GGX distribution, Schlick's Fresnel, Smith–Schlick shadowing.
    let roughness = clamp(input.color.a, 0.03, 0.45);
    let h = normalize(l + v);
    let nh = max(dot(n, h), 0.0);
    let fresnel = WATER_F0 + (1.0 - WATER_F0) * pow(1.0 - max(dot(v, h), 0.0), 5.0);
    let a2 = roughness * roughness * roughness * roughness;
    let d = nh * nh * (a2 - 1.0) + 1.0;
    let distribution = a2 / (PI * d * d);
    let k = roughness * 0.5;
    let shadowing = nl / (nl * (1.0 - k) + k) * nv / (nv * (1.0 - k) + k);
    let glint = distribution * fresnel * shadowing / (4.0 * nv) * water;
    // The water body is seen through the surface, which reflects the share `fresnel`.
    let body = albedo * (1.0 - fresnel * water);
    var color = body * (ambient + sun * nl) + sun * glint;
    // A thin haze towards the limb of the day side (the atmosphere seen at a slant).
    let slant = 1.0 - max(dot(up, v), 0.0);
    let day = smoothstep(-0.1, 0.3, dot(up, l));
    let haze = slant * slant * slant * day;
    let luminance = dot(sun, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = color * (1.0 - 0.4 * haze) + HAZE * haze * luminance;
    return pow(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if uniforms.viewport.w > 0.5 {
        return vec4<f32>(realistic(input, normalize(input.normal)), 1.0);
    }
    let diffuse = max(dot(normalize(input.normal), uniforms.light_direction.xyz), 0.0);
    let ambient = uniforms.light_color.w;
    let lit = vec3<f32>(ambient) + (1.0 - ambient) * diffuse * uniforms.light_color.rgb;
    let light = mix(lit, vec3<f32>(1.0), uniforms.viewport.z);
    return vec4<f32>(min(input.color.rgb * light, vec3<f32>(1.0)), 1.0);
}
