//! The atmosphere of the realistic mode (#92), drawn by `atmosphere.wgsl`: single scattering of
//! sunlight in an exponential atmosphere of Earth-like composition, true to scale (scale height
//! 8 km of a planet with Earth's radius, about a third of a pixel at the view's distance).
//!
//! - **On the disc** (`realistic.wgsl`): the sunlight reaching the ground loses blue light on its
//!   long slant path near the terminator (a reddened terminator), and the air between the ground
//!   and the eye dims the surface and adds its scattered light (aerial perspective: a pale blue
//!   haze towards the limb).
//! - **Off the disc** (`sky.wgsl`): the light scattered along rays that pass the planet: the thin
//!   blue limb glow of the day side, and, with the sun behind the planet, a ring of sunlight
//!   scattered forward by aerosols and air, reddened where it grazed the lower atmosphere.
//!
//! The model: Rayleigh scattering by air (scale height 8 km, vertical optical depth 0.046,
//! 0.108 and 0.265 at 680, 550 and 440 nm) and Mie scattering by aerosols (1.2 km, optical
//! depth 0.05 at 550 nm, Ångström exponent 1, single-scattering albedo 0.9, Cornette–Shanks
//! phase function with g = 0.76). Optical depths to space come from the Chapman function
//! (`sqrt(πx/2) erfcx(μ sqrt(x/2))` with an approximation of erfcx). The simulation has a
//! constant surface pressure of 1 atm (`climate::SURFACE_PRESSURE`) and no parameter of the
//! air's composition, so the coefficients are constants. Not modelled: multiple scattering
//! (the limb and the twilight are somewhat darker than real), ozone absorption, refraction.
//!
//! The atmosphere is lit by the same sunlight and exposure as the surface. It can be far
//! brighter than the exposure allows (the twilight band and the ring around a dark-adapted
//! eclipse), so its colour passes through a hue-preserving shoulder that never reaches white:
//! only the sun, its glow and its glint may clip (#159). See `notes/issue-92/atmosphere.md`.
//!
//! This module holds the model's constants (checked against the shader) and the same model
//! in f64 ([`Model`]) for the exposure meter (the brightest limb) and the tests.

/// Radius of the planet in km: the atmosphere's lengths are in planet radii.
const PLANET_RADIUS_KM: f64 = 6371.0;
/// Scale heights of the air and of the aerosols in km.
const SCALE_HEIGHT_AIR_KM: f64 = 8.0;
const SCALE_HEIGHT_AEROSOL_KM: f64 = 1.2;
/// Rayleigh scattering of the air at the surface per km at 680, 550 and 440 nm (Bruneton and
/// Neyret 2008).
const RAYLEIGH_PER_KM: [f64; 3] = [5.8e-3, 13.5e-3, 33.1e-3];
/// Vertical optical depth of the aerosols at 550 nm, their Ångström exponent (scattering ∝
/// λ^−α) and their single-scattering albedo.
const AEROSOL_DEPTH: f64 = 0.05;
const ANGSTROM: f64 = 1.0;
const AEROSOL_ALBEDO: f64 = 0.9;
/// Wavelengths of the red, green and blue channels in nm.
const WAVELENGTHS: [f64; 3] = [680.0, 550.0, 440.0];
/// Height of the highest top of the atmosphere in scale heights of the air (the shader lowers
/// it at lower exposures, `atmosphere_top`).
const TOP_SCALE_HEIGHTS: f64 = 20.0;

use glam::DVec3;

/// The constants of `atmosphere.wgsl`, in planet radii.
fn shader_constants() -> Vec<(&'static str, Vec<f64>)> {
    let aerosol_at = |wavelength: f64| {
        AEROSOL_DEPTH / SCALE_HEIGHT_AEROSOL_KM
            * (550.0 / wavelength).powf(ANGSTROM)
            * PLANET_RADIUS_KM
    };
    vec![
        (
            "SCALE_HEIGHT_AIR",
            vec![SCALE_HEIGHT_AIR_KM / PLANET_RADIUS_KM],
        ),
        (
            "SCALE_HEIGHT_AEROSOL",
            vec![SCALE_HEIGHT_AEROSOL_KM / PLANET_RADIUS_KM],
        ),
        (
            "RAYLEIGH",
            RAYLEIGH_PER_KM.map(|b| b * PLANET_RADIUS_KM).to_vec(),
        ),
        ("MIE_SCATTERING", WAVELENGTHS.map(aerosol_at).to_vec()),
        (
            "MIE_EXTINCTION",
            WAVELENGTHS.map(|w| aerosol_at(w) / AEROSOL_ALBEDO).to_vec(),
        ),
        (
            "ATMOSPHERE_TOP",
            vec![TOP_SCALE_HEIGHTS * SCALE_HEIGHT_AIR_KM / PLANET_RADIUS_KM],
        ),
    ]
}

/// Asymmetry of the aerosols' phase function (`MIE_G` in the shader).
const MIE_G: f64 = 0.76;
/// The shader's shoulder: the knee and the stops above it that reach half the remaining range.
pub const SHOULDER_KNEE: f64 = 0.86;
#[cfg(test)]
const SHOULDER_STOPS: f64 = 3.0;

/// The shader's model in f64: the exposure's meter uses it for the brightest limb, the tests to
/// check the shader's approximations.
#[derive(Debug, Clone, Copy)]
pub struct Model {
    air: f64,
    aerosol: f64,
    rayleigh: [f64; 3],
    mie_scattering: [f64; 3],
    mie_extinction: [f64; 3],
    top: f64,
    /// Angular radius of the sun in radians.
    sun_radius: f64,
}

impl Model {
    pub fn new(sun_radius: f64) -> Self {
        let constants = shader_constants();
        let get = |name: &str| {
            constants
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, values)| values.clone())
                .unwrap_or_default()
        };
        let three = |name: &str| {
            let v = get(name);
            [v[0], v[1], v[2]]
        };
        Self {
            air: get("SCALE_HEIGHT_AIR")[0],
            aerosol: get("SCALE_HEIGHT_AEROSOL")[0],
            rayleigh: three("RAYLEIGH"),
            mie_scattering: three("MIE_SCATTERING"),
            mie_extinction: three("MIE_EXTINCTION"),
            top: get("ATMOSPHERE_TOP")[0],
            sun_radius,
        }
    }

    /// The column above a point `x` scale heights from the centre towards `mu` (cosine of the
    /// zenith angle), relative to the density at the point, in scale heights (`chapman`).
    fn chapman(x: f64, mu: f64) -> f64 {
        let up = |x: f64, mu: f64| {
            let y = mu * (0.5 * x).sqrt();
            (2.0 * x).sqrt() / (y + (y * y + 4.0 / std::f64::consts::PI).sqrt())
        };
        if mu >= 0.0 {
            return up(x, mu);
        }
        let sine = (1.0 - mu * mu).max(0.0).sqrt();
        let drop = x * mu * mu / (1.0 + sine);
        (2.0 * up(x - drop, 0.0) * drop.min(80.0).exp() - up(x, -mu)).max(0.0)
    }

    fn densities(&self, h: f64) -> (f64, f64) {
        let h = h.max(0.0);
        ((-h / self.air).exp(), (-h / self.aerosol).exp())
    }

    /// Optical depth from radius `r` to space towards `mu`.
    fn optical_depth(&self, r: f64, mu: f64) -> [f64; 3] {
        let (ra, rm) = self.densities(r - 1.0);
        let rr = r.max(1.0);
        let a = self.air * ra * Self::chapman(rr / self.air, mu);
        let m = self.aerosol * rm * Self::chapman(rr / self.aerosol, mu);
        std::array::from_fn(|i| self.rayleigh[i] * a + self.mie_extinction[i] * m)
    }

    /// Transmittance of the sunlight to radius `r` with the sun at `mu`, with the share of the
    /// sun above the horizon.
    fn sun_transmittance(&self, r: f64, mu: f64) -> [f64; 3] {
        // As the shader: asin(mu) ≈ mu, acos(1/r) ≈ sqrt(2h).
        let dip = (2.0 * (r - 1.0).max(0.0)).sqrt();
        let visible = ((mu + dip) / (2.0 * self.sun_radius.max(1e-4)) + 0.5).clamp(0.0, 1.0);
        if visible <= 0.0 {
            return [0.0; 3];
        }
        self.optical_depth(r, mu).map(|t| (-t).exp() * visible)
    }

    /// π × the light scattered towards the eye per unit of sunlight along a ray in direction
    /// `d` (unit) whose tangent point lies at `rt` from the centre in direction `n`, from where
    /// it enters the atmosphere to `end` beyond the tangent point (negative: before it), lit
    /// from `l`, with `samples` points (`scattered_along`).
    pub fn scattered(
        &self,
        n: DVec3,
        rt: f64,
        d: DVec3,
        end: f64,
        l: DVec3,
        samples: usize,
    ) -> [f64; 3] {
        let top = 1.0 + self.top;
        if rt >= top {
            return [0.0; 3];
        }
        let half = ((top - rt) * (top + rt)).sqrt();
        let s_end = end.clamp(-half, half);
        let u_end = s_end.signum() * (s_end.abs() / half).sqrt();
        let du = (u_end + 1.0) / samples as f64;
        let c = l.dot(d);
        let rayleigh_phase = 3.0 / (16.0 * std::f64::consts::PI) * (1.0 + c * c);
        let g2 = MIE_G * MIE_G;
        let denominator = (1.0 + g2 - 2.0 * MIE_G * c).max(1e-4);
        let mie_phase = 3.0 / (8.0 * std::f64::consts::PI) * (1.0 - g2) * (1.0 + c * c)
            / ((2.0 + g2) * denominator * denominator.sqrt());
        let mut sum = [0.0; 3];
        for i in 0..samples {
            let u = -1.0 + (i as f64 + 0.5) * du;
            let s = half * u * u.abs();
            let ds = half * 2.0 * u.abs() * du;
            let x = n * rt + d * s;
            let r = x.length();
            let up = x / r;
            let (ra, rm) = self.densities(r - 1.0);
            let view = self.optical_depth(r, -up.dot(d));
            let sun = self.sun_transmittance(r, up.dot(l));
            for k in 0..3 {
                sum[k] += (self.rayleigh[k] * ra * rayleigh_phase
                    + self.mie_scattering[k] * rm * mie_phase)
                    * (-view[k]).exp()
                    * sun[k]
                    * ds;
            }
        }
        sum.map(|s| s * std::f64::consts::PI)
    }

    /// The limb's brightness (π × scattered light per unit of sunlight) of a ray from `eye`
    /// that passes the planet at the height `height` (planet radii) towards `azimuth` around
    /// the planet as seen from the eye, lit from `sun`.
    pub fn limb(
        &self,
        eye: DVec3,
        sun: DVec3,
        azimuth: f64,
        height: f64,
        samples: usize,
    ) -> [f64; 3] {
        let rt = 1.0 + height;
        if eye.length() <= rt {
            return [0.0; 3];
        }
        let n = self.tangent_point(eye, azimuth, height);
        let d = (n * rt - eye).normalize();
        self.scattered(n, rt, d, 1.0, sun, samples)
    }

    /// The direction from the centre to the tangent point of the ray from `eye` that passes
    /// the planet at `height` towards `azimuth`.
    fn tangent_point(&self, eye: DVec3, azimuth: f64, height: f64) -> DVec3 {
        let distance = eye.length();
        let w = eye / distance;
        let (u, v) = w.any_orthonormal_pair();
        // At the angle acos(rt / distance) from the eye's direction.
        let cosine = ((1.0 + height) / distance).min(1.0);
        let sine = (1.0 - cosine * cosine).sqrt();
        w * cosine + (u * azimuth.cos() + v * azimuth.sin()) * sine
    }

    /// The view direction from `eye` towards the limb at `azimuth` and `height`.
    fn limb_direction(&self, eye: DVec3, azimuth: f64, height: f64) -> DVec3 {
        (self.tangent_point(eye, azimuth, height) * (1.0 + height) - eye).normalize_or_zero()
    }

    /// The brightest point of the limb around the planet seen from `eye` (radii from the
    /// centre) with the sun in `sun`: the largest channel of its brightness per unit of
    /// sunlight (a white surface under the zenith sun is 1). The limb within
    /// [`NEAR_SUN_DEGREES`] of the sun is left out: beside the sun it may clip.
    ///
    /// Conservative against the shader, which averages over a pixel's heights (lowering the
    /// peak) and has a lower top at lower exposures (which matters only far above the peak).
    pub fn brightest_limb(&self, eye: DVec3, sun: DVec3) -> f64 {
        let mut brightest: f64 = 0.0;
        let near_sun = NEAR_SUN_DEGREES.to_radians().cos();
        for step in 0..LIMB_AZIMUTHS {
            let azimuth = std::f64::consts::TAU * step as f64 / LIMB_AZIMUTHS as f64;
            let towards = self.limb_direction(eye, azimuth, 0.0);
            if towards.dot(sun) > near_sun {
                continue;
            }
            for km in LIMB_HEIGHTS_KM {
                let [r, g, b] = self.limb(eye, sun, azimuth, km / PLANET_RADIUS_KM, LIMB_SAMPLES);
                brightest = brightest.max(r).max(g).max(b);
            }
        }
        brightest
    }
}

/// Where the meter looks for the brightest limb: azimuths around the planet, heights of the
/// rays above the surface (the brightest lie within a few scale heights), points along each ray.
const LIMB_AZIMUTHS: usize = 48;
const LIMB_HEIGHTS_KM: [f64; 5] = [0.0, 2.0, 5.0, 10.0, 20.0];
const LIMB_SAMPLES: usize = 12;
/// How close to the sun (seen from the eye) the limb may clip: the meter leaves it out.
const NEAR_SUN_DEGREES: f64 = 10.0;

/// The shader's shoulder (`shoulder`).
#[cfg(test)]
pub fn shoulder(c: [f64; 3]) -> [f64; 3] {
    let m = c[0].max(c[1]).max(c[2]);
    if m <= SHOULDER_KNEE {
        return c;
    }
    let x = (m / SHOULDER_KNEE).log2() / SHOULDER_STOPS;
    let compressed = SHOULDER_KNEE + (1.0 - SHOULDER_KNEE) * x / (1.0 + x);
    c.map(|channel| channel * compressed / m)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUN: f64 = 0.0047;

    impl Model {
        /// Optical depth by numerical integration along the ray (infinite where it hits the
        /// planet).
        fn optical_depth_exact(&self, r: f64, mu: f64) -> [f64; 3] {
            if mu < 0.0 && r * (1.0 - mu * mu).sqrt() < 1.0 {
                return [f64::INFINITY; 3];
            }
            let steps = 100_000;
            let ds = 0.3 / steps as f64;
            let (mut a, mut m) = (0.0, 0.0);
            for i in 0..steps {
                let s = (i as f64 + 0.5) * ds;
                let rr = (r * r + s * s + 2.0 * r * s * mu).sqrt();
                let (ra, rm) = self.densities(rr - 1.0);
                a += ra * ds;
                m += rm * ds;
            }
            std::array::from_fn(|i| self.rayleigh[i] * a + self.mie_extinction[i] * m)
        }

        /// A ray along −x passing the planet at `km` above the surface towards +y.
        fn ray(&self, km: f64, sun: DVec3, samples: usize) -> [f64; 3] {
            self.scattered(
                DVec3::Y,
                1.0 + km / PLANET_RADIUS_KM,
                DVec3::NEG_X,
                1.0,
                sun,
                samples,
            )
        }
    }

    #[test]
    fn the_shader_has_the_same_constants() {
        let shader = include_str!("atmosphere.wgsl");
        let mut names = vec![];
        for (name, values) in shader_constants() {
            let line = shader
                .lines()
                .find(|line| line.starts_with(&format!("const {name}:")))
                .unwrap_or_else(|| panic!("{name} missing"));
            let value_text = line.split('=').nth(1).unwrap();
            let value_text = value_text.rsplit('(').next().unwrap();
            let numbers: Vec<f64> = value_text
                .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == 'e' || c == '-'))
                .filter_map(|word| word.parse().ok())
                .collect();
            assert_eq!(numbers.len(), values.len(), "{line}");
            for (shader_value, value) in numbers.iter().zip(&values) {
                assert!(
                    (shader_value - value).abs() <= 1e-4 * value.abs(),
                    "{name}: {shader_value} vs {value}"
                );
            }
            names.push(name);
        }
        for line in [
            format!("const MIE_G: f32 = {MIE_G:?};"),
            format!("const SHOULDER_KNEE: f32 = {SHOULDER_KNEE:?};"),
            format!("const SHOULDER_STOPS: f32 = {SHOULDER_STOPS:?};"),
        ] {
            assert!(shader.contains(&line), "{line}");
        }
    }

    #[test]
    fn vertical_optical_depths_are_earth_like() {
        let model = Model::new(SUN);
        let depth = model.optical_depth(1.0, 1.0);
        // Air (0.046, 0.108, 0.265) and aerosols (0.05 at 550 nm, Ångström 1, albedo 0.9).
        let expected = [0.046 + 0.0451, 0.108 + 0.0556, 0.265 + 0.0694];
        for (d, e) in depth.iter().zip(expected) {
            assert!((d - e).abs() < 0.002, "{depth:?}");
        }
    }

    #[test]
    fn the_chapman_approximation_is_close() {
        let model = Model::new(SUN);
        for r in [
            1.0,
            1.0 + model.aerosol,
            1.0 + model.air,
            1.0 + 4.0 * model.air,
        ] {
            for mu in [1.0, 0.5, 0.2, 0.1, 0.03, 0.0, -0.01, -0.02, -0.04] {
                let exact = model.optical_depth_exact(r, mu);
                if !exact[0].is_finite() {
                    continue;
                }
                let approximate = model.optical_depth(r, mu);
                for (a, e) in approximate.iter().zip(exact) {
                    assert!((a / e - 1.0).abs() < 0.07, "r {r} μ {mu}: {a} vs {e}");
                }
            }
        }
    }

    #[test]
    fn sunlight_reddens_towards_the_horizon_and_vanishes_below_it() {
        let model = Model::new(SUN);
        let zenith = model.sun_transmittance(1.0, 1.0);
        assert!(zenith[2] > 0.65 && zenith[0] > 0.9, "{zenith:?}");
        let low = model.sun_transmittance(1.0, 0.03);
        assert!(low[0] > 5.0 * low[1] && low[1] > 5.0 * low[2], "{low:?}");
        assert_eq!(model.sun_transmittance(1.0, -0.01), [0.0; 3]);
        // Higher up the sun is still visible below the horizontal.
        let above = model.sun_transmittance(1.0 + 4.0 * model.air, -0.03);
        assert!(above[0] > 0.0, "{above:?}");
    }

    #[test]
    fn the_sampled_limb_converges() {
        let model = Model::new(SUN);
        for sun in [DVec3::X, DVec3::Y, DVec3::new(-1.0, 0.2, 0.0).normalize()] {
            for km in [0.0, 2.0, 5.0, 10.0, 20.0, 40.0] {
                let exact = model.ray(km, sun, 4000);
                for (samples, tolerance) in [(12, 0.15), (8, 0.5)] {
                    let sampled = model.ray(km, sun, samples);
                    for (s, e) in sampled.iter().zip(exact) {
                        assert!(
                            (s - e).abs() <= tolerance * e + 1e-3,
                            "{sun} {km} km {samples}: {sampled:?} vs {exact:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_day_side_limb_is_pale_low_and_blue_high_up_and_does_not_clip() {
        let model = Model::new(SUN);
        // The sun behind the eye: back-scattering.
        let low = model.ray(0.0, DVec3::X, 200);
        let high = model.ray(25.0, DVec3::X, 200);
        assert!(low[2] < 0.5 && low[0] > 0.5 * low[2], "{low:?}");
        assert!(high[2] > 1.5 * high[0], "{high:?}");
    }

    #[test]
    fn aerosols_scatter_forward() {
        let model = Model::new(SUN);
        // Sun beside the limb, 30° behind or in front of the planet: the forward-scattering
        // view is brighter (most in red, which the long path through the air dims least).
        let behind = model.ray(3.0, DVec3::new(-0.87, 0.5, 0.0), 400);
        let front = model.ray(3.0, DVec3::new(0.87, 0.5, 0.0), 400);
        assert!(behind[0] > 1.3 * front[0], "{behind:?} {front:?}");
    }

    #[test]
    fn the_ring_around_the_eclipse_is_red_below_and_blue_above() {
        let model = Model::new(SUN);
        // The eye at 4.5 radii with the sun behind the planet's centre.
        let eye = DVec3::new(4.5, 0.0, 0.0);
        let sun = DVec3::NEG_X;
        let at = |km: f64| model.limb(eye, sun, 0.3, km / PLANET_RADIUS_KM, 400);
        let low = at(3.0);
        let high = at(25.0);
        assert!(low[0] > 3.0 * low[2], "{low:?}");
        assert!(high[2] > high[0], "{high:?}");
        // Dark-adapted (14 stops) the ring is far beyond white; the shoulder keeps it coloured
        // and keeps the gradation between the two.
        let adapted = |c: [f64; 3]| shoulder(c.map(|c| c * 16384.0));
        let (low, high) = (adapted(low), adapted(high));
        assert!(
            low.iter().chain(&high).all(|&c| c < 1.0),
            "{low:?} {high:?}"
        );
        assert!(low[2] < 0.5 * low[0], "{low:?}");
        assert!(high[2] > high[0], "{high:?}");
    }

    #[test]
    fn the_shoulder_keeps_dark_colours_and_never_reaches_white() {
        assert_eq!(shoulder([0.1, 0.5, 0.79]), [0.1, 0.5, 0.79]);
        let mut previous = 0.0;
        for m in [0.81, 1.0, 2.0, 10.0, 100.0, 1e4, 1e6] {
            let c = shoulder([m, 0.5 * m, 0.25 * m]);
            assert!(c[0] < 1.0 && c[0] > previous, "{c:?}");
            assert!((c[1] / c[0] - 0.5).abs() < 1e-9);
            previous = c[0];
        }
        // Gradation stays: 4 stops above the knee still differ visibly from 8.
        let at = |stops: f64| shoulder([SHOULDER_KNEE * stops.exp2(), 0.0, 0.0])[0];
        assert!(at(8.0) - at(4.0) > 0.015, "{} {}", at(4.0), at(8.0));
    }

    #[test]
    fn the_brightest_limb() {
        let model = Model::new(SUN);
        let eye = DVec3::new(4.5, 0.0, 0.0);
        // Sun behind the eye: the back-scattering limb, below the knee.
        let day = model.brightest_limb(eye, DVec3::X);
        assert!(day > 0.05 && day < SHOULDER_KNEE, "{day}");
        // The crescent of #159 (40° from the eclipse): forward scattering, brighter.
        let angle = 40f64.to_radians();
        let crescent = model.brightest_limb(eye, DVec3::new(-angle.cos(), angle.sin(), 0.0));
        assert!(crescent > day, "{crescent} {day}");
    }
}
