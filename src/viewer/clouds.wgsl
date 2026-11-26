// The clouds of the realistic mode (#92, stage 5): a decoration derived from the climate, not
// simulated. Appended to `realistic.wgsl`, whose noise it uses; see `modes/realistic.rs`
// (`cloud_layer`) for the cover, type, optical depth and height of every cell, and
// `notes/issue-92/clouds.md`.
//
// The pattern belongs to the atmosphere, so it is anchored to the world frame (it does not ride
// on the plates), and it is static: the same frame always shows the same clouds. Its first
// octave is a synoptic "pressure" field ψ (lows cloudy, highs clear), whose geostrophic flow
// swirls the finer octaves around the lows and highs; the field is thresholded at the quantile
// of the cell's cover, so that the cloudy share of the pixels is the cover.

// Features are this many times longer east–west than north–south at low latitudes (bands of
// the ITCZ, trade cumulus, fronts), fading to round at the poles: the noise is evaluated at
// (x, y, CLOUD_ZONAL · z).
const CLOUD_ZONAL: f32 = 1.6;
// Frequency of the pressure field ψ per planet radius (a wavelength of ~3000 km).
const CLOUD_PRESSURE_FREQUENCY: f32 = 2.0;
// Amplitude of ψ relative to the first finer octave.
const CLOUD_PRESSURE_WEIGHT: f32 = 0.6;
// Largest displacement of the finer octaves along the flow around ψ, in planet radii per unit
// of ψ's slope.
const CLOUD_SWIRL: f32 = 0.012;
// The standard deviation of one octave of `noise` (raw).
const NOISE_DEVIATION: f32 = 0.19;
// Asymmetry of the scattering by cloud droplets.
const CLOUD_G: f32 = 0.85;
// Most reflectance of a cloud: below the shoulder's knee (0.86, fresh snow), so that no cloud is
// brighter than the brightest surface and the exposure's cap holds.
const CLOUD_MAX_REFLECTANCE: f32 = 0.84;
// The most reflectance of a cloud together with the ground seen through it (the shoulder's
// knee).
const CLOUD_KNEE: f32 = 0.86;
// Planet radii per km.
const CLOUD_KM: f32 = 1.0 / 6371.0;

// The cloud field at the point `u` of the unit sphere: its value normalised to a standard
// deviation of 1 (x) and its gradient with respect to `u` in planet radii (yzw, tangential
// only in part: the caller projects it). `octaves` octaves (fractional) after ψ, the first at
// `base` cycles per radius, each `persistence` times the amplitude of the one before; ψ's
// amplitude is scaled by `synoptic` (less for fields of small cumulus).
fn cloud_field(
    u: vec3<f32>,
    base: f32,
    octaves: f32,
    persistence: f32,
    synoptic: f32,
) -> vec4<f32> {
    let stretch = vec3<f32>(1.0, 1.0, CLOUD_ZONAL);
    let q = u * stretch;
    let pressure = noise(q * CLOUD_PRESSURE_FREQUENCY + vec3<f32>(3.7, -1.3, 8.9));
    // The gradient of ψ with respect to u.
    let slope = pressure.yzw * stretch * (CLOUD_PRESSURE_FREQUENCY / NOISE_DEVIATION);
    // Lows (ψ < 0) are cloudy: ψ enters with the opposite sign.
    let pressure_weight = CLOUD_PRESSURE_WEIGHT * synoptic;
    var sum = vec4<f32>(-pressure.x / NOISE_DEVIATION, -slope) * pressure_weight;
    var variance = pressure_weight * pressure_weight;
    // The geostrophic flow along the isobars, counter-clockwise around lows in the north and
    // clockwise in the south; no balance near the equator. The finer octaves are advected along
    // it, which bends them around the lows and highs into swirls.
    let tangential = slope - dot(slope, u) * u;
    let flow = cross(u, tangential) * sign(u.z) * smoothstep(0.15, 0.45, abs(u.z));
    let warped = (u - CLOUD_SWIRL * flow) * stretch;
    var amplitude = 1.0;
    var frequency = base;
    for (var octave = 0; octave < MAX_OCTAVES + 1; octave++) {
        let weight = clamp(octaves - f32(octave), 0.0, 1.0);
        if weight <= 0.0 {
            break;
        }
        let n = noise(warped * frequency + vec3<f32>(f32(octave) * 5.17 - 11.0));
        let a = amplitude * weight;
        sum += vec4<f32>(n.x, n.yzw * stretch * frequency) * (a / NOISE_DEVIATION);
        variance += a * a;
        amplitude *= persistence;
        frequency *= 2.0;
    }
    return sum * inverseSqrt(variance);
}

// The quantile of the standard normal distribution at `p` (logistic approximation, within
// ~0.1 for p in 0.02..0.98).
fn normal_quantile(p: f32) -> f32 {
    return 0.5513 * log(p / (1.0 - p));
}

// Reflection of a non-absorbing cloud of optical depth `tau` lit at the zenith cosine `mu0`:
// two-stream (Bohren 1987), `x / (2 + x)` with the scaled optical depth `x = (1 − g) τ`, made
// longer by the slant path of a low sun. At most CLOUD_MAX_REFLECTANCE.
fn cloud_reflectance(tau: f32, mu0: f32) -> f32 {
    let x = (1.0 - CLOUD_G) * tau * 0.5 * (1.0 + 1.0 / max(mu0, 0.15));
    return min(x / (2.0 + x), CLOUD_MAX_REFLECTANCE);
}

// The optical depth of the cloud at `u` for a cell's cloud data `cell` (see `VertexOutput`),
// with the field `field`; `cover` is the cover after the limb's increase, `footprint` the size
// of a pixel in planet radii (for anti-aliasing the edges).
fn cloud_depth(field: vec4<f32>, u: vec3<f32>, cell: vec4<f32>, cover: f32, footprint: f32) -> f32 {
    let threshold = -normal_quantile(clamp(cover, 0.002, 0.998));
    let excess = field.x - threshold;
    // The change of the field across a pixel, so that an edge is never sharper than a pixel.
    let tangential = field.yzw - dot(field.yzw, u) * u;
    let blur = length(tangential) * footprint;
    // Soft ramps for sheets, crisp edges for convective clouds (in standard deviations).
    let ramp = mix(1.2, 0.3, cell.y) + blur;
    let density = smoothstep(-0.5 * blur, ramp, excess);
    // Thin at the edges, thicker towards the cores.
    return cell.z * 64.0 * density * 1.6 * (0.35 + 0.65 * smoothstep(0.0, 2.0, excess));
}
