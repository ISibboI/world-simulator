// The atmosphere of the realistic mode (#92): single scattering of sunlight in an exponential
// atmosphere of Earth-like composition around the planet (radius 1), true to scale. Prepended
// to `realistic.wgsl` (the haze on the disc and the reddened sunlight) and to `sky.wgsl` (the
// limb glow and the ring around the backlit planet off the disc); see `atmosphere.rs`, which
// holds the same model for the tests, and `notes/issue-92/atmosphere.md`.
//
// Units: lengths in planet radii. Light: the shader's sunlight makes a white Lambertian surface
// lit at normal incidence show the sunlight itself, so scattered radiance shows as
// `sunlight × π × ∫ β ρ P T_view T_sun ds`.

// Scale heights of the air (Rayleigh) and of the aerosols (Mie): 8 km and 1.2 km.
const SCALE_HEIGHT_AIR: f32 = 0.0012557;
const SCALE_HEIGHT_AEROSOL: f32 = 0.00018835;
// Rayleigh scattering of the air at the surface per planet radius for red, green and blue
// (680, 550, 440 nm): (5.8, 13.5, 33.1) per 1000 km, a vertical optical depth of
// (0.046, 0.108, 0.265).
const RAYLEIGH: vec3<f32> = vec3<f32>(36.952, 86.009, 210.885);
// Aerosol scattering at the surface per planet radius (a vertical optical depth of 0.05 at
// 550 nm, Ångström exponent 1), and its extinction (single-scattering albedo 0.9).
const MIE_SCATTERING: vec3<f32> = vec3<f32>(214.71, 265.46, 331.82);
const MIE_EXTINCTION: vec3<f32> = vec3<f32>(238.57, 294.95, 368.69);
// Asymmetry of the aerosols' phase function (forward scattering).
const MIE_G: f32 = 0.76;
// Highest top of the atmosphere: 20 scale heights of the air (160 km), where the density is
// 2e-9 of the surface's. See `atmosphere_top`.
const ATMOSPHERE_TOP: f32 = 0.025113;
const ATMOSPHERE_PI: f32 = 3.14159265;

// The column of a species above a point at `x` scale heights from the planet's centre, towards
// a direction `mu` (cosine of the zenith angle, ≥ 0), relative to the density at the point, in
// scale heights: the Chapman function, as `sqrt(πx/2) erfcx(mu sqrt(x/2))` with an
// approximation of erfcx (within 7% of the exact column for x > 700, see `atmosphere.rs`).
fn chapman_up(x: f32, mu: f32) -> f32 {
    let y = mu * sqrt(0.5 * x);
    return sqrt(2.0 * x) / (y + sqrt(y * y + 1.2732395));
}

// The same for any direction: a ray below the horizon (`mu` < 0) passes a tangent point lower
// down and climbs out again. The result is relative to the density at the point. `x` is the
// point's distance from the centre in scale heights.
fn chapman(x: f32, mu: f32) -> f32 {
    if mu >= 0.0 {
        return chapman_up(x, mu);
    }
    // The tangent point is lower by x (1 − sin χ) = x μ² / (1 + sin χ) scale heights.
    let sine = sqrt(max(1.0 - mu * mu, 0.0));
    let drop = x * mu * mu / (1.0 + sine);
    // (Never negative, also where the tangent point would lie deep inside the planet.)
    return max(2.0 * chapman_up(x - drop, 0.0) * exp(min(drop, 80.0)) - chapman_up(x, -mu), 0.0);
}

// Density of the air (x) and of the aerosols (y) at height `h` above the surface, relative to
// the surface (heights below the surface count as the surface).
fn densities(h: f32) -> vec2<f32> {
    let height = max(h, 0.0);
    return exp(-vec2<f32>(height / SCALE_HEIGHT_AIR, height / SCALE_HEIGHT_AEROSOL));
}

// Optical depth (extinction) from a point at distance `r` from the centre to space towards a
// direction with cosine of the zenith angle `mu`.
fn optical_depth(r: f32, mu: f32) -> vec3<f32> {
    let rho = densities(r - 1.0);
    let rr = max(r, 1.0);
    let air = SCALE_HEIGHT_AIR * rho.x * chapman(rr / SCALE_HEIGHT_AIR, mu);
    let aerosol = SCALE_HEIGHT_AEROSOL * rho.y * chapman(rr / SCALE_HEIGHT_AEROSOL, mu);
    return RAYLEIGH * air + MIE_EXTINCTION * aerosol;
}

// The share of the sun's disc (angular radius `sun_radius`) above the horizon of a point at
// height `h` above the surface, where the sun stands at `mu` (cosine of its zenith angle): its
// elevation, asin(mu) ≈ mu, against the horizon's dip, acos(1/(1 + h)) ≈ sqrt(2h) (both within 1%
// where the sun is near the horizon in the atmosphere; elsewhere the share is 0 or 1 anyway).
fn sun_visibility(h: f32, mu: f32, sun_radius: f32) -> f32 {
    let dip = sqrt(2.0 * max(h, 0.0));
    return clamp((mu + dip) / (2.0 * max(sun_radius, 1e-4)) + 0.5, 0.0, 1.0);
}

// Transmittance of the sunlight to a point at distance `r` from the centre with the sun at
// `mu`, including the share of the sun above the horizon.
fn sun_transmittance(r: f32, mu: f32, sun_radius: f32) -> vec3<f32> {
    let visible = sun_visibility(r - 1.0, mu, sun_radius);
    if visible <= 0.0 {
        return vec3<f32>(0.0);
    }
    return exp(-optical_depth(r, mu)) * visible;
}

// The height of the top of the atmosphere for the exposure's gain `gain` (in linear light): the
// air above it adds less than 1/1000 of white. 10 scale heights (80 km) in daylight, one more
// per factor e of gain, at most ATMOSPHERE_TOP (a dark-adapted eclipse). Rays above it are
// skipped, so in daylight the limb costs about half as many pixels and points.
fn atmosphere_top(gain: f32) -> f32 {
    return min(SCALE_HEIGHT_AIR * (10.0 + log(max(gain, 1.0))), ATMOSPHERE_TOP);
}

// Phase functions (per steradian) for the cosine of the scattering angle `c` (1 is forward).
fn rayleigh_phase(c: f32) -> f32 {
    return 3.0 / (16.0 * ATMOSPHERE_PI) * (1.0 + c * c);
}

fn mie_phase(c: f32) -> f32 {
    let g2 = MIE_G * MIE_G;
    let denominator = max(1.0 + g2 - 2.0 * MIE_G * c, 1e-4);
    return 3.0 / (8.0 * ATMOSPHERE_PI) * (1.0 - g2) * (1.0 + c * c)
        / ((2.0 + g2) * denominator * sqrt(denominator));
}

// Scales a colour in linear light so that no channel reaches 1 while keeping its hue: a soft
// shoulder above SHOULDER_KNEE (the brightest reflectance, fresh snow, so that no surface is
// touched), logarithmic, so that light many stops brighter than the knee
// keeps its gradation. The atmosphere can be far brighter than the exposure allows (the twilight
// band and the ring around a dark-adapted eclipse); it then shows saturated, never white.
fn shoulder(c: vec3<f32>) -> vec3<f32> {
    let m = max(c.r, max(c.g, c.b));
    if m <= SHOULDER_KNEE {
        return c;
    }
    let x = log2(m / SHOULDER_KNEE) / SHOULDER_STOPS;
    let compressed = SHOULDER_KNEE + (1.0 - SHOULDER_KNEE) * x / (1.0 + x);
    return c * (compressed / m);
}

const SHOULDER_KNEE: f32 = 0.86;
// Stops above the knee that reach half of the remaining range.
const SHOULDER_STOPS: f32 = 3.0;

// The atmosphere at a point `p` of the surface seen from the eye `e` (outside the atmosphere),
// with the light from `l`:
// - `sunlight`: the sunlight on the ground relative to its light under the sun at the zenith:
//   dimmer and redder towards the terminator, and zero once the sun has set. Relative to the
//   zenith because the sky's own light (the multiple scattering this model leaves out) makes up
//   for most of the blue the direct sunlight loses by day, and the eye adapts its white to
//   daylight;
// - `transmittance` towards the eye;
// - `inscattered`: the light scattered towards the eye per unit of sunlight before the factor
//   π. Closed form where the eye looks down steeply: the sunlight's transmittance is taken as
//   constant along the short path, at one scale height of the air above the point (its columns
//   towards the sun scaled from the ground's). Near the limb, where the path is long and
//   crosses a few degrees of the sun's elevation (the terminator's width), it is integrated like
//   the rays that pass the planet (`samples` points), so the disc meets the limb glow without a
//   seam.
struct SurfaceAir {
    sunlight: vec3<f32>,
    transmittance: vec3<f32>,
    inscattered: vec3<f32>,
};

const AERIAL_NUMERIC_FROM: f32 = 0.12;
const AERIAL_NUMERIC_TO: f32 = 0.2;
// The sun's zenith cosines below which the ground and the air just above it (the closed form),
// and the air along a grazing path near the limb (up to the top of the atmosphere, a few
// degrees away), get no sunlight.
const NIGHT_GROUND: f32 = -0.06;
const NIGHT_LIMB: f32 = -0.3;
// The density of the air and of the aerosols one scale height of the air higher: e^−1 and
// e^−(8 / 1.2).
const LIFTED: vec2<f32> = vec2<f32>(0.36788, 0.0012726);

fn surface_air(
    p: vec3<f32>,
    e: vec3<f32>,
    l: vec3<f32>,
    sun_radius: f32,
    top_height: f32,
    samples: i32,
) -> SurfaceAir {
    let r = max(length(p), 1.0);
    let h = r - 1.0;
    let up = normalize(p);
    let to_eye = e - p;
    let distance = length(to_eye);
    let v = to_eye / distance;
    let mu = max(dot(up, v), 0.0);
    let mu_sun = dot(up, l);
    let rho = densities(h);
    let x = vec2<f32>(r / SCALE_HEIGHT_AIR, r / SCALE_HEIGHT_AEROSOL);
    let scale = vec2<f32>(SCALE_HEIGHT_AIR, SCALE_HEIGHT_AEROSOL) * rho;
    // Columns towards the eye.
    let view = scale * vec2<f32>(chapman_up(x.x, mu), chapman_up(x.y, mu));
    let extinction = RAYLEIGH * view.x + MIE_EXTINCTION * view.y;
    var result: SurfaceAir;
    result.transmittance = exp(-extinction);
    result.sunlight = vec3<f32>(0.0);
    result.inscattered = vec3<f32>(0.0);
    var closed = vec3<f32>(0.0);
    if mu_sun > NIGHT_GROUND {
        // Columns towards the sun and towards the zenith (where the Chapman function is 1).
        let towards_sun = scale * vec2<f32>(chapman(x.x, mu_sun), chapman(x.y, mu_sun));
        let zenith = RAYLEIGH * scale.x + MIE_EXTINCTION * scale.y;
        result.sunlight = exp(zenith - RAYLEIGH * towards_sun.x - MIE_EXTINCTION * towards_sun.y)
            * sun_visibility(h, mu_sun, sun_radius);
        if mu > AERIAL_NUMERIC_FROM || samples <= 0 {
            let lifted = towards_sun * LIFTED;
            let sun = exp(-RAYLEIGH * lifted.x - MIE_EXTINCTION * lifted.y)
                * sun_visibility(h + SCALE_HEIGHT_AIR, mu_sun, sun_radius);
            // The scattering angle's cosine: the light travels along −l, then towards the eye.
            let c = dot(v, -l);
            let scattering = RAYLEIGH * (view.x * rayleigh_phase(c))
                + MIE_SCATTERING * (view.y * mie_phase(c));
            closed = sun * scattering / max(extinction, vec3<f32>(1e-6))
                * (1.0 - result.transmittance);
        }
    }
    if mu >= AERIAL_NUMERIC_TO || samples <= 0 || mu_sun < NIGHT_LIMB {
        result.inscattered = closed;
        return result;
    }
    let d = -v;
    let t = -dot(e, d);
    let rt = length(cross(e, d));
    let n = (e + d * t) / max(rt, 1e-6);
    let numeric = scattered_along(n, rt, d, t, distance, l, sun_radius, top_height, samples);
    result.inscattered = mix(
        numeric,
        closed,
        smoothstep(AERIAL_NUMERIC_FROM, AERIAL_NUMERIC_TO, mu),
    );
    return result;
}

// Light scattered towards the eye along a ray from the eye `e` (outside the atmosphere) in
// direction `d` (unit), lit from `l`, per unit of sunlight before the factor π, up to the
// distance `end` along the ray (the ground, or beyond the atmosphere for a ray that misses the
// planet), with `samples` points. `rt` is the ray's distance from the centre at its tangent
// point (its closest approach), `n` the direction of that point. The points are denser towards
// the tangent point, where the air is densest: s = L u |u| from the tangent point, L the half
// chord through the top of the atmosphere.
fn scattered_along(
    n: vec3<f32>,
    rt: f32,
    d: vec3<f32>,
    t_tangent: f32,
    end: f32,
    l: vec3<f32>,
    sun_radius: f32,
    top_height: f32,
    samples: i32,
) -> vec3<f32> {
    let top = 1.0 + top_height;
    if rt >= top {
        return vec3<f32>(0.0);
    }
    let half = sqrt((top - rt) * (top + rt));
    // The end as u: s_end = L u_end |u_end|.
    let s_end = clamp(end - t_tangent, -half, half);
    let u_end = sign(s_end) * sqrt(abs(s_end) / half);
    let du = (u_end + 1.0) / f32(samples);
    let c = dot(l, d);
    let phase = vec2<f32>(rayleigh_phase(c), mie_phase(c));
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < samples; i++) {
        let u = -1.0 + (f32(i) + 0.5) * du;
        let s = half * u * abs(u);
        let ds = half * 2.0 * abs(u) * du;
        let x = n * rt + d * s;
        let r = length(x);
        let up = x / r;
        let rho = densities(r - 1.0);
        let sun = sun_transmittance(r, dot(up, l), sun_radius);
        let view = exp(-optical_depth(r, -dot(up, d)));
        let scattering = RAYLEIGH * (rho.x * phase.x) + MIE_SCATTERING * (rho.y * phase.y);
        sum += scattering * view * sun * ds;
    }
    return sum;
}

// The atmosphere of a ray that misses the planet (the limb glow, and the ring around a backlit
// planet): the light scattered towards the eye per unit of sunlight before the factor π (rgb of
// `inscattered`), and the transmittance along the ray (for the stars and the sun behind it). The
// result is averaged over the heights the pixel covers at the tangent point above the surface
// (`pixel`: its size in radians), `heights` of them, each integrated with `samples` points.
const SILHOUETTE_SAG: f32 = 0.998;

struct Limb {
    inscattered: vec3<f32>,
    transmittance: vec3<f32>,
};

fn limb_light(
    e: vec3<f32>,
    d: vec3<f32>,
    l: vec3<f32>,
    sun_radius: f32,
    pixel: f32,
    top_height: f32,
    heights: i32,
    samples: i32,
) -> Limb {
    var result: Limb;
    result.inscattered = vec3<f32>(0.0);
    result.transmittance = vec3<f32>(1.0);
    let t = -dot(e, d);
    // The distance of the ray from the centre (precise also where it nearly grazes).
    let rc = length(cross(e, d));
    let footprint = pixel * max(t, 0.0);
    // Skipped above the top, and well inside the planet's disc (where the planet covers the
    // pixel; the margin, ~13 km, covers the mesh's silhouette lying inside the sphere).
    if t <= 0.0 || rc > 1.0 + top_height + 0.5 * footprint || rc + 0.5 * footprint < SILHOUETTE_SAG
        || heights <= 0 {
        return result;
    }
    let n = (e + d * t) / max(rc, 1e-6);
    var inscattered = vec3<f32>(0.0);
    var transmittance = vec3<f32>(0.0);
    // The heights the pixel covers above the surface (the part below is the planet's, which
    // multisampling blends in on the desktop). The mesh's silhouette lies a little inside the
    // sphere between its vertices: a ray that the planet does not cover passes at least at the
    // surface.
    let low = max(rc - 0.5 * footprint, 1.0);
    let high = max(rc + 0.5 * footprint, low);
    for (var k = 0; k < heights; k++) {
        let rt = mix(low, high, (f32(k) + 0.5) / f32(heights));
        inscattered += scattered_along(n, rt, d, t, t + 1.0, l, sun_radius, top_height, samples);
        transmittance += exp(-2.0 * optical_depth(rt, 0.0));
    }
    result.inscattered = inscattered / f32(heights);
    result.transmittance = transmittance / f32(heights);
    return result;
}
