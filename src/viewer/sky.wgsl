// Draws the sun and the stars behind the planet: a full-screen triangle just in front of the far
// plane, depth-tested against the planet (which hides them where it is in front), added to the
// background. The sun's disc has its true angular size (from the sun model), limb darkening, and
// a glow around it as a camera would show it. The stars are a procedural field fixed in space,
// as bright as the camera's exposure lets them be (see `sky.rs`).

struct Uniforms {
    view_projection: mat4x4<f32>,
    light_direction: vec4<f32>,
    viewport: vec4<f32>,
    // Inverse of view_projection: from normalised device coordinates to the world.
    inverse_view_projection: mat4x4<f32>,
    // Unit vector towards the sun (xyz) and the angular radius of its disc in radians (w).
    sun_direction: vec4<f32>,
    // Colour of the sun's disc in sRGB (rgb) and the share of it the planet does not hide (w).
    sun_color: vec4<f32>,
    light_color: vec4<f32>,
    // Visibility of the stars (x, 0 to 1) and the size of a pixel in radians (y).
    stars: vec4<f32>,
    // Black-body colours of the stars from 3200 K to 20000 K (see STAR_TEMPERATURES in sky.rs).
    star_colors: array<vec4<f32>, 8>,
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
    // The glow fades as the planet covers the disc (the disc itself is cut by the depth test).
    let intensity = mix(glow * uniforms.sun_color.w, limb, disc);
    var color = uniforms.sun_color.rgb * intensity;
    if uniforms.stars.x > 0.0 {
        color += stars(direction) * uniforms.stars.x;
    }
    return vec4<f32>(color, 0.0);
}

// Cells of the star field per edge of a cube face: about 0.5° wide.
const STAR_CELLS: u32 = 180u;
// Mean stars per cell: ~15000 over the sky down to the limiting magnitude.
const STARS_PER_CELL: f32 = 0.077;
// Limiting magnitude, and the magnitude that fills a pixel with white.
const FAINTEST: f32 = 7.0;
const SATURATED: f32 = 1.5;
// Growth of the star counts per magnitude: N(< m) ∝ 10^(0.45 m).
const COUNT_SLOPE: f32 = 0.45;

// PCG hash (Jarkko & Olano, 2020).
fn hash(value: u32) -> u32 {
    let state = value * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn unit(value: u32) -> f32 {
    return f32(value >> 8u) / 16777216.0;
}

// The point (u, v) in [-1, 1]² of cube face `face` (0–2: +x, +y, +z; 3–5: −x, −y, −z) as a
// direction (not normalised).
fn face_point(face: u32, u: f32, v: f32) -> vec3<f32> {
    let sign = select(1.0, -1.0, face >= 3u);
    let axis = face % 3u;
    if axis == 0u {
        return vec3<f32>(sign, u, v);
    }
    if axis == 1u {
        return vec3<f32>(v, sign, u);
    }
    return vec3<f32>(u, v, sign);
}

// The colour (sRGB, before the visibility) of the stars at `direction`: every cell of the cube
// face around it and its neighbours holds a star or none, at a random place in the cell, with a
// random magnitude and temperature.
fn stars(direction: vec3<f32>) -> vec3<f32> {
    let a = abs(direction);
    var face: u32;
    var uv: vec2<f32>;
    if a.x >= a.y && a.x >= a.z {
        face = select(0u, 3u, direction.x < 0.0);
        uv = direction.yz / a.x;
    } else if a.y >= a.z {
        face = select(1u, 4u, direction.y < 0.0);
        uv = direction.zx / a.y;
    } else {
        face = select(2u, 5u, direction.z < 0.0);
        uv = direction.xy / a.z;
    }
    let cells = f32(STAR_CELLS);
    let cell = vec2<i32>(clamp(floor((uv * 0.5 + 0.5) * cells), vec2<f32>(0.0), vec2<f32>(cells - 1.0)));
    let pixel = uniforms.stars.y;
    var color = vec3<f32>(0.0);
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let c = cell + vec2<i32>(dx, dy);
            if c.x < 0 || c.y < 0 || c.x >= i32(STAR_CELLS) || c.y >= i32(STAR_CELLS) {
                continue;
            }
            var h = hash(face * STAR_CELLS * STAR_CELLS + u32(c.y) * STAR_CELLS + u32(c.x) + 0x9e3779b9u);
            // Cells near the corners of a face cover a smaller solid angle: fewer stars.
            let centre = ((vec2<f32>(c) + 0.5) / cells) * 2.0 - 1.0;
            let area = pow(1.0 + dot(centre, centre), -1.5);
            // 1.9: the mean of `area` over a face is its solid angle per area of [-1, 1]²,
            // (4π/6) / 4 = 0.524, so that the cells hold STARS_PER_CELL stars on average.
            if unit(h) >= STARS_PER_CELL * area * 1.9 {
                continue;
            }
            h = hash(h);
            let x = 0.2 + 0.6 * unit(h);
            h = hash(h);
            let y = 0.2 + 0.6 * unit(h);
            let star = normalize(face_point(face, ((f32(c.x) + x) / cells) * 2.0 - 1.0, ((f32(c.y) + y) / cells) * 2.0 - 1.0));
            h = hash(h);
            let magnitude = FAINTEST + log2(max(unit(h), 1e-7)) * 0.30103 / COUNT_SLOPE;
            h = hash(h);
            // Temperature: 3200 K (M), 4000 and 4800 K (K, mostly giants), G, F, A, late and
            // early B (see STAR_TEMPERATURES), weighted like the
            // naked-eye stars.
            let t = unit(h);
            var kind = 7u;
            if t < 0.08 { kind = 0u; } else if t < 0.30 { kind = 1u; } else if t < 0.47 { kind = 2u; }
            else if t < 0.59 { kind = 3u; } else if t < 0.70 { kind = 4u; } else if t < 0.82 { kind = 5u; }
            else if t < 0.93 { kind = 6u; }
            let flux = exp2((SATURATED - magnitude) * 0.4 * 3.321928);
            // A point spread of about a pixel, wider for stars that overflow it (as in photos).
            let sigma = pixel * clamp(0.55 + 0.25 * log2(max(flux, 1.0)), 0.55, 1.6);
            let distance = length(cross(direction, star));
            let value = flux * exp(-0.5 * distance * distance / (sigma * sigma));
            color += uniforms.star_colors[kind].rgb * pow(min(value, 1.0), 1.0 / 2.2);
        }
    }
    return color;
}
