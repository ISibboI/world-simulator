// The realistic mode's surface (#92): the layers of every cell (see `modes/realistic.rs`) mixed
// per pixel with sub-cell detail, lit in linear light by the simulation's sun.
//
// The mesh is de-indexed: every triangle has its own three vertices, each knowing its corner
// (for barycentric coordinates) and the plates of all three corners. The detail is gradient
// noise in the frame of the plate of the nearest corner (cell centre), so it moves with the
// crust from frame to frame. Noise that moves a boundary (coast, snow line, tree line) is
// weighted to zero at the corners, so every cell centre keeps the class its data gives. The
// number of octaves follows the size of a pixel on the surface (fewer near the limb and on small
// screens): level of detail per pixel, and anti-aliasing without multisampling.

struct Uniforms {
    view_projection: mat4x4<f32>,
    // Direction towards the light, in world space (w unused).
    light_direction: vec4<f32>,
    viewport: vec4<f32>,
    inverse_view_projection: mat4x4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    // Colour of the light as a factor on the sRGB colours (rgb), share of ambient light (w).
    light_color: vec4<f32>,
    stars: vec4<f32>,
    star_colors: array<vec4<f32>, 8>,
    // The camera's position (xyz).
    eye: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct Detail {
    // Orientation of every plate: the rotation from its frame into the world, (x, y, z, w).
    orientations: array<vec4<f32>, 256>,
    // Frequency of the first octave per planet radius (x), most octaves (y), strength of the
    // bump relief (z), ridged mountains (w: 0 or 1).
    settings: vec4<f32>,
};

@group(0) @binding(1)
var<uniform> detail: Detail;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // Bare ground (sRGB) and the plant cover (a).
    @location(2) bare: vec4<f32>,
    // Full plant cover (sRGB) and the snow or ice cover (a).
    @location(3) plants: vec4<f32>,
    // Snow or ice (sRGB) and the water's roughness / 0.5 (a).
    @location(4) snow: vec4<f32>,
    // Open water (sRGB) and the ruggedness of the relief (a).
    @location(5) water: vec4<f32>,
    // Positive on land, negative on water.
    @location(6) coast: f32,
    // The plates of the triangle's corners (xyz) and this vertex's corner (w).
    @location(7) plates: vec4<u32>,
    // Per corner of the triangle: the strike of a mountain belt (snorm8 xyz, world frame), the
    // squeeze of the relief noise along it (bits 24–30, 0 to 0.75) and the ocean (bit 31).
    @location(8) ridges: vec3<u32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    // The position (xyz) and the vertex's distance from the centre (w), which, interpolated,
    // follows the surface instead of the flat triangle's chord.
    @location(1) position: vec4<f32>,
    @location(2) bare: vec4<f32>,
    @location(3) plants: vec4<f32>,
    @location(4) snow: vec4<f32>,
    @location(5) water: vec4<f32>,
    // Barycentric coordinates (xyz) and the coast value (w).
    @location(6) corner_coast: vec4<f32>,
    @location(7) @interpolate(flat) plates: vec3<u32>,
    @location(8) @interpolate(flat) ridges: vec3<u32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = uniforms.view_projection * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.position = vec4<f32>(input.position, length(input.position));
    output.bare = input.bare;
    output.plants = input.plants;
    output.snow = input.snow;
    output.water = input.water;
    let corner = input.plates.w;
    output.corner_coast = vec4<f32>(
        select(0.0, 1.0, corner == 0u),
        select(0.0, 1.0, corner == 1u),
        select(0.0, 1.0, corner == 2u),
        input.coast,
    );
    output.plates = input.plates.xyz;
    output.ridges = input.ridges;
    return output;
}

const PI: f32 = 3.14159265;
// Reflectance of water at normal incidence (refractive index 1.33).
const WATER_F0: f32 = 0.02;
// Most octaves of the noise.
const MAX_OCTAVES: i32 = 6;
// Scale of the fractal sum (see `fbm`).
const FBM_SCALE: f32 = 2.7;

// Exact sRGB transfer functions (the CPU encodes the reflectances with the same curve).
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((max(c, vec3<f32>(0.0)) + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c <= vec3<f32>(0.0031308));
}

fn pow5(x: f32) -> f32 {
    let x2 = x * x;
    return x2 * x2 * x;
}

// Rotates `v` by the unit quaternion `q`.
fn rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}

// A pseudo-random gradient in [-1, 1]³ for a lattice point (an integer hash, the same on every
// GPU).
fn gradient(p: vec3<i32>) -> vec3<f32> {
    var v = bitcast<vec3<u32>>(p) * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v ^= v >> vec3<u32>(16u);
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return vec3<f32>(v >> vec3<u32>(8u)) / 8388608.0 - 1.0;
}

// Gradient noise with its analytic derivatives (Quílez): value (x) and gradient (yzw).
fn noise(x: vec3<f32>) -> vec4<f32> {
    let i = floor(x);
    let f = x - i;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);
    let p = vec3<i32>(i);
    let ga = gradient(p);
    let gb = gradient(p + vec3<i32>(1, 0, 0));
    let gc = gradient(p + vec3<i32>(0, 1, 0));
    let gd = gradient(p + vec3<i32>(1, 1, 0));
    let ge = gradient(p + vec3<i32>(0, 0, 1));
    let gf = gradient(p + vec3<i32>(1, 0, 1));
    let gg = gradient(p + vec3<i32>(0, 1, 1));
    let gh = gradient(p + vec3<i32>(1, 1, 1));
    let va = dot(ga, f);
    let vb = dot(gb, f - vec3<f32>(1.0, 0.0, 0.0));
    let vc = dot(gc, f - vec3<f32>(0.0, 1.0, 0.0));
    let vd = dot(gd, f - vec3<f32>(1.0, 1.0, 0.0));
    let ve = dot(ge, f - vec3<f32>(0.0, 0.0, 1.0));
    let vf = dot(gf, f - vec3<f32>(1.0, 0.0, 1.0));
    let vg = dot(gg, f - vec3<f32>(0.0, 1.0, 1.0));
    let vh = dot(gh, f - vec3<f32>(1.0, 1.0, 1.0));
    let value = va + u.x * (vb - va) + u.y * (vc - va) + u.z * (ve - va)
        + u.x * u.y * (va - vb - vc + vd) + u.y * u.z * (va - vc - ve + vg)
        + u.z * u.x * (va - vb - ve + vf)
        + u.x * u.y * u.z * (-va + vb + vc - vd + ve - vf - vg + vh);
    let derivative = ga + u.x * (gb - ga) + u.y * (gc - ga) + u.z * (ge - ga)
        + u.x * u.y * (ga - gb - gc + gd) + u.y * u.z * (ga - gc - ge + gg)
        + u.z * u.x * (ga - gb - ge + gf)
        + u.x * u.y * u.z * (-ga + gb + gc - gd + ge - gf - gg + gh)
        + du * (vec3<f32>(vb - va, vc - va, ve - va)
            + u.yzx * vec3<f32>(va - vb - vc + vd, va - vc - ve + vg, va - vb - ve + vf)
            + u.zxy * vec3<f32>(va - vb - ve + vf, va - vb - vc + vd, va - vc - ve + vg)
            + u.yzx * u.zxy * (-va + vb + vc - vd + ve - vf - vg + vh));
    return vec4<f32>(value, derivative);
}

// The signed byte `byte` (0 to 2) of `word` in [-1, 1] (snorm8; GLSL ES 3.00 has no
// unpackSnorm4x8).
fn snorm8(word: u32, byte: u32) -> f32 {
    return max(f32(bitcast<i32>(word << (24u - 8u * byte)) >> 24u) / 127.0, -1.0);
}

// The squeeze of a corner's relief noise along its strike.
fn corner_squeeze(word: u32) -> f32 {
    return f32((word >> 24u) & 127u) * (0.75 / 127.0);
}

// Fractal sum of `octaves` (fractional: the last one fades in) octaves of noise at `x`: value
// (x) and gradient with respect to `x` (yzw), scaled so that the value has a standard deviation
// of about 0.4 (|value| > 1 in about 1% of the pixels; the raw sum's is about 0.15).
fn fbm(x: vec3<f32>, octaves: f32) -> vec4<f32> {
    var sum = vec4<f32>(0.0);
    var amplitude = 0.7;
    var frequency = 1.0;
    for (var octave = 0; octave < MAX_OCTAVES; octave++) {
        let weight = clamp(octaves - f32(octave), 0.0, 1.0);
        if weight > 0.0 {
            let n = noise(x * frequency + vec3<f32>(f32(octave) * 7.31));
            sum += vec4<f32>(n.x, n.yzw * frequency) * amplitude * weight;
        }
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    return sum * FBM_SCALE;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let up = normalize(input.position.xyz);
    // Derivatives first, while the control flow is uniform: the size of a pixel on the
    // surface in planet radii (its longer side), and how fast the coast value changes across it.
    let footprint = max(length(dpdx(up)), length(dpdy(up)));
    let coast_width = fwidth(input.corner_coast.w);

    // The nearest corner, its plate and the plate's frame.
    let b = input.corner_coast.xyz;
    var corner = 0u;
    if b.y > b.x && b.y >= b.z {
        corner = 1u;
    } else if b.z > b.x && b.z > b.y {
        corner = 2u;
    }
    let plate = input.plates[corner];
    var orientation = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if plate < 256u {
        orientation = detail.orientations[plate];
    }
    let inverse = vec4<f32>(-orientation.xyz, orientation.w);
    let anchor = rotate(inverse, up);
    // Zero at the corners (cell centres), one in the middle of the triangle.
    let weight = 1.0 - smoothstep(0.6, 1.0, max(b.x, max(b.y, b.z)));

    // Octaves below the pixel's Nyquist limit (frequency × footprint < 0.5), at most the
    // quality's.
    let base = detail.settings.x;
    let available = log2(0.5 / max(base * footprint, 1e-6)) + 1.0;
    let octaves = clamp(available, 0.0, detail.settings.y);
    let shape = fbm(anchor * base, octaves);
    // A second, independent pattern for snow, plants and colour; the Low level (at most two
    // octaves) reuses the first, which halves its cost.
    var pattern = -shape.x;
    if detail.settings.y > 2.5 {
        pattern = fbm(anchor * base + vec3<f32>(17.3, -9.1, 5.7), max(octaves - 1.0, 0.0)).x;
    }
    let n_shape = clamp(shape.x, -1.0, 1.0);
    let n_pattern = clamp(pattern, -1.0, 1.0);

    // Mountain ridges along the strike of a belt (with the bump relief, Medium and High): each
    // corner with a strike has its own noise field, squeezed along its strike (ridges up to
    // four times as long as wide), the others share `shape`; the fields are blended by the
    // squared barycentric weights. A corner's field is fixed by its cell, so the blend is
    // continuous across the triangles' edges; neighbouring corners of a belt have nearly the
    // same field, so the blend keeps its contrast there.
    var relief = shape;
    let squeezes = vec3<f32>(
        corner_squeeze(input.ridges.x),
        corner_squeeze(input.ridges.y),
        corner_squeeze(input.ridges.z),
    );
    if detail.settings.z > 0.0 && max(squeezes.x, max(squeezes.y, squeezes.z)) > 0.0 {
        let y = anchor * base;
        let w = b * b;
        var sum = vec4<f32>(0.0);
        for (var j = 0; j < 3; j++) {
            if w[j] <= 0.0 {
                continue;
            }
            if squeezes[j] > 0.0 {
                let word = input.ridges[j];
                let strike = vec3<f32>(snorm8(word, 0u), snorm8(word, 1u), snorm8(word, 2u));
                let along = rotate(inverse, normalize(strike));
                let n = fbm(y - squeezes[j] * dot(y, along) * along, octaves);
                // The gradient with respect to `y`: the squeeze is symmetric.
                let gradient = n.yzw - squeezes[j] * dot(n.yzw, along) * along;
                sum += w[j] * vec4<f32>(n.x, gradient);
            } else {
                sum += w[j] * shape;
            }
        }
        relief = sum / (w.x + w.y + w.z);
    }
    // The height the bump relief implies, on which the snow lies first: the noise, or on High
    // −|noise| (its crests where the noise crosses zero), recentred and scaled to the same
    // spread (|n| of a normal spread 0.4 has a mean of 0.32 and a spread of 0.24).
    let ridged_height = (0.32 - abs(relief.x)) * (0.4 / 0.24);
    let height = clamp(mix(relief.x, ridged_height, detail.settings.w), -1.0, 1.0);

    // Coast: the coast value with noise, anti-aliased over a pixel.
    // (At most ±0.5 against corner values of ±0.4 to ±1: islands and lakes can appear only
    // where the land is low or the water shallow, never in the deep ocean.)
    let coast = input.corner_coast.w + weight * 0.5 * n_shape;
    let edge = coast_width + weight * 2.0 * base * footprint + 1e-4;
    let land = smoothstep(-edge, edge, coast);
    // Snow line and plant cover: partial covers break up into patches; full and no cover stay.
    // Snow and ice have crisp edges: the snow cover is sharpened around one half. On rugged
    // land the snow lies on the crests of the relief first. Sea ice belongs to the climate,
    // not to the crust under it: its pattern is anchored to the world frame.
    let snow_cover = input.plants.a;
    // (Two independent patterns mixed by t keep their spread when divided by
    // sqrt(t² + (1 − t)²).)
    // (Low draws no relief, and its pattern is the relief's noise itself.)
    let rugged = select(0.0, input.water.a, detail.settings.z > 0.0);
    var n_snow = mix(n_pattern, height, rugged) * inverseSqrt(1.0 - 2.0 * rugged * (1.0 - rugged));
    let sea = dot(b, vec3<f32>(input.ridges >> vec3<u32>(31u)));
    if sea > 0.0 && snow_cover > 0.01 && snow_cover < 0.99 {
        // Floes and leads finer than the cells' pattern: from the second octave on.
        let floes = clamp(
            fbm(up * (2.0 * base) + vec3<f32>(-23.1, 11.9, 4.3), max(octaves - 1.0, 0.0)).x,
            -1.0,
            1.0,
        );
        n_snow = mix(n_snow, floes, sea) * inverseSqrt(1.0 - 2.0 * sea * (1.0 - sea));
    }
    n_snow = clamp(n_snow, -1.0, 1.0);
    let snow = smoothstep(
        0.2,
        0.8,
        snow_cover + weight * 3.2 * snow_cover * (1.0 - snow_cover) * n_snow,
    );
    let plant_cover = input.bare.a;
    let plants = clamp(
        plant_cover + weight * 2.4 * plant_cover * (1.0 - plant_cover) * (n_pattern - n_shape) * 0.7,
        0.0,
        1.0,
    );
    // Colour variation of the ground (it does not change the class).
    let variation = 1.0 + 0.15 * (0.5 * n_shape + 0.5 * n_pattern);
    let ground = mix(srgb_to_linear(input.bare.rgb), srgb_to_linear(input.plants.rgb), plants)
        * variation;
    let snow_albedo = srgb_to_linear(input.snow.rgb);
    let land_albedo = mix(ground, snow_albedo, snow);
    let water_albedo = mix(srgb_to_linear(input.water.rgb), snow_albedo, snow);
    let albedo = mix(water_albedo, land_albedo, land);
    // Open water (not ice) reflects the sun.
    let water = (1.0 - land) * (1.0 - snow);

    // Bump relief: the noise's gradient tilts the normal of rugged land.
    let mesh_normal = normalize(input.normal);
    var slope = rotate(orientation, relief.yzw);
    slope -= dot(slope, up) * up;
    let ridged = detail.settings.w * select(1.0, -1.0, relief.x > 0.0);
    // Every octave adds as much slope: normalised by their number, so that the relief does not
    // get rougher with more pixels per cell.
    let tilt = detail.settings.z * input.water.a * mix(1.0, ridged, detail.settings.w)
        / sqrt(max(octaves, 1.0));
    let land_normal = normalize(mesh_normal - tilt * slope);
    let n = normalize(mix(land_normal, up, 1.0 - land));

    // Lighting in linear light: the light's colour carries the exposure's gain as a factor on
    // sRGB values (2^(stops/2.2), see `sky::Adaptation`).
    let l = uniforms.light_direction.xyz;
    let v = normalize(uniforms.eye.xyz - input.position.xyz);
    let sun = pow(max(uniforms.light_color.rgb, vec3<f32>(0.0)), vec3<f32>(2.2));
    let ambient = pow(uniforms.light_color.w, 2.2);
    // The exposure's gain in linear light (`stars.z` is its factor on sRGB values). No lit
    // surface may over-expose (#159): the view caps the exposure at the brightest incidence on
    // the smooth sphere, and this caps the incidence of the bump-tilted normals the same way,
    // so that a reflectance never shows brighter than under the zenith sun in daylight.
    let gain = pow(max(uniforms.stars.z, 1e-6), 2.2);
    let nl = min(max(dot(n, l), 0.0), 1.0 / gain);
    let nv = max(dot(n, v), 1e-3);
    // Sun glint: GGX distribution, Schlick's Fresnel, Smith–Schlick shadowing.
    let roughness = clamp(input.snow.a * 0.5, 0.03, 0.45);
    let h = normalize(l + v + vec3<f32>(0.0, 0.0, 1e-6));
    let nh = max(dot(n, h), 0.0);
    let fresnel = WATER_F0 + (1.0 - WATER_F0) * pow5(1.0 - max(dot(v, h), 0.0));
    let a2 = roughness * roughness;
    let d = nh * nh * (a2 - 1.0) + 1.0;
    let distribution = a2 / (PI * d * d);
    let k = roughness * 0.5;
    let shadowing = nl / (nl * (1.0 - k) + k) * nv / (nv * (1.0 - k) + k);
    let glint = distribution * fresnel * shadowing / (4.0 * nv) * water;
    // The water body is seen through the surface, which reflects the Fresnel share of the view.
    let reflected = WATER_F0 + (1.0 - WATER_F0) * pow5(1.0 - nv);
    let body = albedo * (1.0 - reflected * water);
    // The atmosphere (`atmosphere.wgsl`; `viewport.w` is the number of points along a ray, 0
    // without an atmosphere): the sunlight reaching the ground reddens towards the terminator,
    // and the air between the ground and the eye dims the surface and adds its own light.
    // The flat triangles between the cell centres sag up to a few km below the surface, a scale
    // height of the aerosols: the air is measured from the interpolated radius of the corners
    // (at least the sea surface), or the mesh would show through the haze at grazing views.
    let samples = i32(uniforms.viewport.w);
    let surface_point = up * max(input.position.w, 1.0);
    var direct = sun;
    var air: SurfaceAir;
    if samples > 0 {
        air = surface_air(
            surface_point,
            uniforms.eye.xyz,
            l,
            uniforms.sun_direction.w,
            atmosphere_top(gain),
            samples,
        );
        direct = sun * air.sunlight;
    }
    var color = body * (ambient + direct * nl);
    var mirrored = direct * glint;
    if samples > 0 {
        // Nothing but the sun's glint may reach white, and the atmosphere can be far brighter
        // than the exposure allows (the twilight band, a crescent's haze): a hue-preserving
        // shoulder on everything but the glint.
        color = shoulder(color * air.transmittance + sun * ATMOSPHERE_PI * air.inscattered);
        mirrored *= air.transmittance;
    }
    color += mirrored;
    return vec4<f32>(linear_to_srgb(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
