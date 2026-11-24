//! The sun of the planet: a Sun-like star that brightens on the main sequence (#92).
//!
//! [`Sun::at_age`] gives the state of the star at any age from its zero-age main sequence (the
//! planet's first oceans and plates form a few hundred Myr later) to 10 Gyr, about 5.4 Gyr after
//! today and before it leaves the main sequence for the subgiant and red-giant branches:
//!
//! - **luminosity** from Gough (1981), `L(t) = L₀ / (1 + 0.4 (1 − t/t₀))`, which the climate
//!   already uses for its solar constant ([`crate::presets::Epoch`]): 0.71 L₀ at the zero age,
//!   0.77 L₀ at 3.5 Ga, 1.29 L₀ at 7.1 Gyr and 1.90 L₀ at 10 Gyr (detailed models: 0.70, 0.77,
//!   1.26 and 1.84; Bahcall et al., 2001; Schröder & Smith, 2008);
//! - **effective temperature** interpolated linearly in age between the points of the solar
//!   model of Schröder & Smith (2008): 5596 K at the zero age, 5772 K today (IAU 2015 nominal),
//!   5820 K at its hottest (7.13 Gyr), 5745 K at 10 Gyr;
//! - **radius** from the two by the Stefan–Boltzmann law, `R/R₀ = √(L/L₀) (T₀/T)²`: 0.90 R₀ at
//!   the zero age, 1.39 R₀ at 10 Gyr (Schröder & Smith: 0.89 and 1.37);
//! - **colour** of a black body of the effective temperature in linear sRGB.
//!
//! The simulation does not change the sun during a run: the climate keeps the solar constant of
//! its epoch (`--epoch`). The view shows the sun of the displayed time (the epoch's age plus the
//! simulated time), whose luminosity grows by ~1% per 100 Myr. Sources and checks:
//! `notes/issue-92/sun.md`.

use glam::DVec3;

use crate::math;
use crate::temperature::OrbitParams;

/// Today's solar constant in W/m² (Kopp & Lean, 2011).
pub const PRESENT_SOLAR_CONSTANT: f64 = 1361.0;
/// Age of the Sun in Gyr (Bouvier & Wadhwa, 2010: 4.568 Gyr for the oldest solids).
pub const SOLAR_AGE: f64 = 4.57;
/// Today's effective temperature of the Sun in K (IAU 2015 Resolution B3).
pub const PRESENT_EFFECTIVE_TEMPERATURE: f64 = 5772.0;
/// Today's radius of the Sun in m (IAU 2015 Resolution B3).
pub const PRESENT_RADIUS: f64 = 6.957e8;
/// The astronomical unit in m, the semi-major axis of the planet's orbit: the distance at which
/// today's Sun gives [`PRESENT_SOLAR_CONSTANT`].
pub const ASTRONOMICAL_UNIT: f64 = 1.495_978_707e11;
/// The oldest age in Gyr the model describes; older ages give this state. The Sun leaves the
/// main sequence at 10–11 Gyr (Schröder & Smith, 2008), where these fits end.
pub const MAX_AGE: f64 = 10.0;

/// Effective temperature (K) of the solar model of Schröder & Smith (2008) by age (Gyr): zero
/// age, today (the IAU value instead of their 5774 K), the hottest point and 10 Gyr.
const TEMPERATURE_BY_AGE: [(f64, f64); 4] = [
    (0.0, 5596.0),
    (SOLAR_AGE, PRESENT_EFFECTIVE_TEMPERATURE),
    (7.13, 5820.0),
    (MAX_AGE, 5745.0),
];

/// The solar constant (W/m² at one astronomical unit) at an age of the Sun in Gyr, from the
/// brightening on the main sequence (Gough, 1981): `S(t) = S₀ / (1 + 0.4 (1 − t/t₀))`.
pub fn solar_constant(age: f64) -> f64 {
    PRESENT_SOLAR_CONSTANT / (1.0 + 0.4 * (1.0 - age / SOLAR_AGE))
}

/// The state of the sun at some age.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    /// Age in Gyr since the zero-age main sequence, within `0..=MAX_AGE`.
    pub age: f64,
    /// Luminosity relative to today's.
    pub luminosity: f64,
    /// Effective temperature in K.
    pub effective_temperature: f64,
    /// Radius relative to today's.
    pub radius: f64,
}

impl Sun {
    /// The sun at an age in Gyr, clamped to `0..=MAX_AGE`.
    pub fn at_age(age: f64) -> Self {
        let age = if age.is_nan() {
            SOLAR_AGE
        } else {
            age.clamp(0.0, MAX_AGE)
        };
        let luminosity = solar_constant(age) / PRESENT_SOLAR_CONSTANT;
        let effective_temperature = effective_temperature(age);
        let ratio = PRESENT_EFFECTIVE_TEMPERATURE / effective_temperature;
        Self {
            age,
            luminosity,
            effective_temperature,
            radius: luminosity.sqrt() * ratio * ratio,
        }
    }

    /// The sun of a run that started `before_present` Gyr ago (`--epoch`), `time` Myr into the
    /// run.
    pub fn of_run(before_present: f64, time: f64) -> Self {
        Self::at_age(SOLAR_AGE - before_present + time / 1000.0)
    }

    /// Irradiance in W/m² at `distance` (relative to the semi-major axis, one astronomical unit).
    pub fn irradiance(&self, distance: f64) -> f64 {
        PRESENT_SOLAR_CONSTANT * self.luminosity / (distance * distance)
    }

    /// Angular radius of the disc in radians, seen from `distance` (relative to the semi-major
    /// axis): 4.65 mrad (0.267°) for today's Sun at one astronomical unit.
    pub fn angular_radius(&self, distance: f64) -> f64 {
        math::asin((PRESENT_RADIUS * self.radius / (ASTRONOMICAL_UNIT * distance)).min(1.0))
    }

    /// Colour of the light: a black body of the effective temperature in linear sRGB (D65
    /// white), scaled to a luminance (CIE Y) of 1. Slightly warmer than white: about (1.07,
    /// 0.98, 0.91) today, (1.11, 0.97, 0.85) at the zero age.
    pub fn color(&self) -> [f64; 3] {
        black_body_color(self.effective_temperature)
    }
}

/// Effective temperature in K at an age in Gyr (`0..=MAX_AGE`), interpolated linearly in
/// [`TEMPERATURE_BY_AGE`].
fn effective_temperature(age: f64) -> f64 {
    let points = &TEMPERATURE_BY_AGE;
    for pair in points.windows(2) {
        let ((a0, t0), (a1, t1)) = (pair[0], pair[1]);
        if age <= a1 {
            return t0 + (t1 - t0) * ((age - a0) / (a1 - a0)).clamp(0.0, 1.0);
        }
    }
    points[points.len() - 1].1
}

/// Linear sRGB colour (D65) of a black body at `temperature` K, scaled to a luminance of 1: the
/// Planck spectrum from 380 to 780 nm in 5 nm steps, weighted with the CIE 1931 colour-matching
/// functions in the multi-lobe Gaussian fit of Wyman, Sloan & Shirley (2013).
pub fn black_body_color(temperature: f64) -> [f64; 3] {
    /// Second radiation constant `hc/k` in m·K.
    const C2: f64 = 1.438_777e-2;
    // A lobe of the fit: piecewise Gaussian around `mean` (nm), widths below and above.
    let lobe = |wavelength: f64, mean: f64, below: f64, above: f64| {
        let width = if wavelength < mean { below } else { above };
        let x = (wavelength - mean) / width;
        math::exp(-0.5 * x * x)
    };
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    for step in 0..=80 {
        let wavelength = 380.0 + 5.0 * f64::from(step);
        let metres = wavelength * 1e-9;
        // Planck's law up to a constant factor (it cancels in the normalisation).
        let radiance = 1.0 / (metres.powi(5) * math::exp_m1(C2 / (metres * temperature)));
        x += radiance
            * (1.056 * lobe(wavelength, 599.8, 37.9, 31.0)
                + 0.362 * lobe(wavelength, 442.0, 16.0, 26.7)
                - 0.065 * lobe(wavelength, 501.1, 20.4, 26.2));
        y += radiance
            * (0.821 * lobe(wavelength, 568.8, 46.9, 40.5)
                + 0.286 * lobe(wavelength, 530.9, 16.3, 31.1));
        z += radiance
            * (1.217 * lobe(wavelength, 437.0, 11.8, 36.0)
                + 0.681 * lobe(wavelength, 459.0, 26.0, 13.8));
    }
    let (x, z) = (x / y, z / y);
    // CIE XYZ to linear sRGB (IEC 61966-2-1).
    [
        3.2406 * x - 1.5372 - 0.4986 * z,
        -0.9689 * x + 1.8758 + 0.0415 * z,
        0.0557 * x - 0.2040 + 1.0570 * z,
    ]
}

/// Direction from the planet's centre towards the sun in the planet's frame (the rotation axis
/// is +z), with the sun over `longitude` (radians; the simulated planet does not spin, so this
/// is a fixed time of day) and the declination of the orbital position at `fraction` (0..1) of
/// the year after perihelion, or 0 (an equinox) for `None`, the annual view. Also returns the
/// distance to the sun relative to the semi-major axis (1 for `None`).
pub fn direction(orbit: &OrbitParams, longitude: f64, fraction: Option<f64>) -> (DVec3, f64) {
    let (declination, distance) = match fraction {
        Some(fraction) => {
            let position = orbit.position(fraction);
            (position.declination, position.distance)
        }
        None => (0.0, 1.0),
    };
    let (sin_dec, cos_dec) = math::sin_cos(declination);
    let (sin_lon, cos_lon) = math::sin_cos(longitude);
    (
        DVec3::new(cos_dec * cos_lon, cos_dec * sin_lon, sin_dec),
        distance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_is_the_present_sun() {
        let sun = Sun::at_age(SOLAR_AGE);
        assert_eq!(sun.luminosity, 1.0);
        assert_eq!(sun.effective_temperature, PRESENT_EFFECTIVE_TEMPERATURE);
        assert!((sun.radius - 1.0).abs() < 1e-12);
        assert!((sun.irradiance(1.0) - PRESENT_SOLAR_CONSTANT).abs() < 1e-9);
        // 0.2666° (16.0 arcmin) at one astronomical unit.
        let degrees = sun.angular_radius(1.0).to_degrees();
        assert!((degrees - 0.2665).abs() < 0.001, "{degrees}");
    }

    #[test]
    fn young_and_old_suns_match_detailed_models() {
        // Schröder & Smith (2008): zero age 0.70 L, 5596 K, 0.89 R; 10 Gyr 1.84 L, 1.37 R.
        let young = Sun::at_age(0.0);
        assert!((young.luminosity - 0.714).abs() < 0.001);
        assert_eq!(young.effective_temperature, 5596.0);
        assert!((young.radius - 0.89).abs() < 0.015, "{}", young.radius);
        let old = Sun::at_age(MAX_AGE);
        assert!((old.luminosity - 1.84).abs() < 0.08, "{}", old.luminosity);
        assert!((old.radius - 1.37).abs() < 0.03, "{}", old.radius);
        // Beyond the range: clamped.
        assert_eq!(Sun::at_age(-1.0), young);
        assert_eq!(Sun::at_age(20.0), old);
        assert_eq!(Sun::at_age(f64::NAN), Sun::at_age(SOLAR_AGE));
    }

    #[test]
    fn the_sun_brightens_and_grows_monotonically() {
        let mut previous = Sun::at_age(0.0);
        for step in 1..=100 {
            let sun = Sun::at_age(MAX_AGE * f64::from(step) / 100.0);
            assert!(sun.luminosity > previous.luminosity);
            assert!(sun.radius > previous.radius);
            previous = sun;
        }
    }

    #[test]
    fn a_run_ages_the_sun_with_simulated_time() {
        // `--epoch 3.5` and 500 Myr into the run: 1.57 Gyr old.
        let sun = Sun::of_run(3.5, 500.0);
        assert!((sun.age - 1.57).abs() < 1e-12);
        assert_eq!(Sun::of_run(0.0, 0.0), Sun::at_age(SOLAR_AGE));
    }

    #[test]
    fn black_bodies_have_their_known_colours() {
        // D65 is close to a black body of 6504 K: nearly white.
        let white = black_body_color(6504.0);
        for channel in white {
            assert!((channel - 1.0).abs() < 0.05, "{white:?}");
        }
        // The sun is slightly warmer than white; cooler bodies redder.
        let sun = Sun::at_age(SOLAR_AGE).color();
        assert!(sun[0] > sun[1] && sun[1] > sun[2], "{sun:?}");
        assert!(sun[2] > 0.85, "{sun:?}");
        let young = Sun::at_age(0.0).color();
        assert!(young[2] / young[0] < sun[2] / sun[0]);
        let red = black_body_color(3000.0);
        assert!(red[2] < 0.3 * red[0], "{red:?}");
    }

    #[test]
    fn the_sun_stands_at_the_declination_of_the_season() {
        let orbit = OrbitParams::default();
        let (annual, distance) = direction(&orbit, 0.0, None);
        assert_eq!(distance, 1.0);
        assert!((annual - DVec3::X).length() < 1e-12);
        // Early January: southern summer, near perihelion.
        let (january, distance) = direction(&orbit, 1.0, Some(0.5 / 12.0));
        assert!(january.z < -0.35 && distance < 0.99);
        assert!((january.length() - 1.0).abs() < 1e-12);
        let (july, _) = direction(&orbit, 1.0, Some(6.5 / 12.0));
        assert!(july.z > 0.35);
    }
}
