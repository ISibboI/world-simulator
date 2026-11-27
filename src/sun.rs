//! The sun of the planet: a Sun-like star that brightens on the main sequence (#92).
//!
//! [`Sun::at_age`] gives the state of the star at any age from its zero-age main sequence (the
//! earliest start of a simulation, epoch 0, is [`EPOCH_ZERO_AGE`] = 0.57 Gyr later) to 10 Gyr,
//! about 5.4 Gyr after today and before it leaves the main sequence for the subgiant and
//! red-giant branches:
//!
//! - **luminosity** from Gough (1981), `L(t) = L₀ / (1 + 0.4 (1 − t/t₀))`, which the climate
//!   already uses for its solar constant ([`crate::presets::Epoch`]): 0.71 L₀ at the zero age,
//!   0.77 L₀ at 3.5 Ga, 1.29 L₀ at 7.13 Gyr and 1.91 L₀ at 10 Gyr (detailed models: 0.70, 0.77,
//!   1.26 and 1.84; Bahcall et al., 2001; Schröder & Smith, 2008);
//! - **effective temperature** interpolated linearly in age between the points of the solar
//!   model of Schröder & Smith (2008): 5596 K at the zero age, 5772 K today (IAU 2015 nominal),
//!   5820 K at its hottest (7.13 Gyr), 5745 K at 10 Gyr;
//! - **radius** from the two by the Stefan–Boltzmann law, `R/R₀ = √(L/L₀) (T₀/T)²`: 0.90 R₀ at
//!   the zero age, 1.39 R₀ at 10 Gyr (Schröder & Smith: 0.89 and 1.37);
//! - **colour** of a black body of the effective temperature in linear sRGB.
//!
//! The sun brightens during a run by ~1% per 100 Myr: the climate scales its solar constant
//! with [`brightening`] at every update (#149), and the view shows the sun of the displayed
//! time. Both take the age of the sun from [`age_of_run`], the one place that converts the
//! epoch (`--epoch`) and the simulated time to the age of the sun. Sources and checks:
//! `notes/issue-92/sun.md`, `notes/issue-149/`.
//!
//! **Epochs** (#150) count forwards in Gyr: epoch 0 is the earliest start of a simulation, when
//! the planet has cooled enough for plates and oceans, [`EPOCH_ZERO_AGE`] = 0.57 Gyr after the
//! sun's zero age (4.0 Ga, the end of the Hadean on Earth); [`PRESENT_EPOCH`] (4.0) is today, and
//! [`MAX_EPOCH`] (5.90) the latest time a run may reach: the sun is then [`MAX_LUMINOSITY`] = 1.2
//! times as bright as today, beyond which an ocean planet enters a runaway greenhouse that the
//! model does not simulate (#212). See `notes/issue-150/plan.md`. A run starts at epoch 0 unless
//! `--epoch` says otherwise (#190). The sun model itself goes on to [`MAX_AGE`] (the view's sun).

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
/// The epoch of today: the time in Gyr from the earliest start of a simulation (epoch 0) to the
/// present, 4.0 Ga (the oldest preserved crust, the Acasta Gneiss, is 4.03 Gyr old; Bowring &
/// Williams, 1999).
pub const PRESENT_EPOCH: f64 = 4.0;
/// The earliest start of a simulation, epoch 0 (4.0 Ga), and the default start of a run (#190).
pub const EARLIEST_EPOCH: f64 = 0.0;
/// The age of the sun in Gyr at epoch 0, the earliest start of a simulation.
pub const EPOCH_ZERO_AGE: f64 = SOLAR_AGE - PRESENT_EPOCH;
/// The brightest sun a run may see, relative to today's: beyond ~1.1–1.2 S₀ an ocean planet's
/// water vapour traps more heat than it can radiate and the ocean boils into a steam atmosphere,
/// the runaway greenhouse (Kasting, 1988; Kopparapu et al., 2013: 1.06 S₀ in 1-D; 3-D models
/// with clouds: ~1.1–1.2 S₀, Leconte et al., 2013; Wolf & Toon, 2015). The model does not simulate it (its ocean
/// stays liquid at any temperature), so no run goes past it (#212).
pub const MAX_LUMINOSITY: f64 = 1.2;
/// The age of the sun in Gyr at [`MAX_LUMINOSITY`]: the inverse of [`solar_constant`],
/// `t = t₀ (1 − (1/L − 1)/0.4)`, 6.47 Gyr.
pub const MAX_LUMINOSITY_AGE: f64 = SOLAR_AGE * (1.0 - (1.0 / MAX_LUMINOSITY - 1.0) / 0.4);
/// The latest epoch a run may reach, at [`MAX_LUMINOSITY`]: 5.90 (1.9 Gyr from today).
pub const MAX_EPOCH: f64 = MAX_LUMINOSITY_AGE - EPOCH_ZERO_AGE;

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

/// The age of the sun in Gyr in a run that started at `epoch` (`--epoch`, Gyr after the
/// earliest start; see the module documentation), `time` Myr into the run. The only place that
/// converts the epoch and the simulated time to the sun's age; not clamped (see [`Sun::at_age`]
/// and [`brightening`]).
///
/// Computed through the time before present, `PRESENT_EPOCH − epoch` (exact for the usual
/// epochs, e.g. 0.5 → 3.5 Ga), as before #150, so that a run keeps its results bit for bit
/// under the new numbering of its epoch.
pub fn age_of_run(epoch: f64, time: f64) -> f64 {
    SOLAR_AGE - (PRESENT_EPOCH - epoch) + time / 1000.0
}

/// The factor by which the sun is brighter at the age `age` (Gyr) than at `start`, both clamped
/// to `0..=MAX_AGE` as in [`Sun::at_age`]: `S(age) / S(start)`, exactly 1 for `age == start`.
/// The climate multiplies its solar constant of the run's start by it.
pub fn brightening(start: f64, age: f64) -> f64 {
    solar_constant(age.clamp(0.0, MAX_AGE)) / solar_constant(start.clamp(0.0, MAX_AGE))
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

    /// The sun of a run that started at `epoch` (`--epoch`), `time` Myr into the run.
    pub fn of_run(epoch: f64, time: f64) -> Self {
        Self::at_age(age_of_run(epoch, time))
    }

    /// Angular radius of the disc in radians, seen from `distance` (relative to the semi-major
    /// axis): 4.65 mrad (0.267°) for today's Sun at one astronomical unit.
    pub fn angular_radius(&self, distance: f64) -> f64 {
        math::asin((PRESENT_RADIUS * self.radius / (ASTRONOMICAL_UNIT * distance)).min(1.0))
    }

    /// Colour of the light: a black body of the effective temperature in linear sRGB (D65
    /// white), scaled to a luminance (CIE Y) of 1. Slightly warmer than white: about (1.11,
    /// 0.98, 0.91) today, (1.13, 0.97, 0.88) at the zero age.
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
    fn brightening_is_relative_to_the_start() {
        for start in [0.0, 1.07, SOLAR_AGE, 9.9] {
            assert_eq!(brightening(start, start), 1.0);
        }
        // ~0.9% per 100 Myr today, and beyond the model the sun stays as at its end.
        let today = brightening(SOLAR_AGE, SOLAR_AGE + 0.1);
        assert!((1.008..1.010).contains(&today), "{today}");
        assert_eq!(brightening(9.0, 12.0), brightening(9.0, MAX_AGE));
        assert_eq!(age_of_run(PRESENT_EPOCH, 0.0), SOLAR_AGE);
        assert!((age_of_run(0.5, 1000.0) - (SOLAR_AGE - 2.5)).abs() < 1e-12);
    }

    #[test]
    fn today_is_the_present_sun() {
        let sun = Sun::at_age(SOLAR_AGE);
        assert_eq!(sun.luminosity, 1.0);
        assert_eq!(sun.effective_temperature, PRESENT_EFFECTIVE_TEMPERATURE);
        assert!((sun.radius - 1.0).abs() < 1e-12);
        assert_eq!(solar_constant(sun.age), PRESENT_SOLAR_CONSTANT);
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
        // `--epoch 0.5` (3.5 Ga) and 500 Myr into the run: 1.57 Gyr old.
        let sun = Sun::of_run(0.5, 500.0);
        assert!((sun.age - 1.57).abs() < 1e-12);
        assert_eq!(Sun::of_run(PRESENT_EPOCH, 0.0), Sun::at_age(SOLAR_AGE));
    }

    #[test]
    fn epochs_count_forwards_from_the_earliest_start() {
        assert!((EPOCH_ZERO_AGE - 0.57).abs() < 1e-12);
        assert!((MAX_EPOCH - 5.904).abs() < 1e-3, "{MAX_EPOCH}");
        assert_eq!(age_of_run(0.0, 0.0), EPOCH_ZERO_AGE);
        assert!((age_of_run(MAX_EPOCH, 0.0) - MAX_LUMINOSITY_AGE).abs() < 1e-12);
        let luminosity = Sun::of_run(MAX_EPOCH, 0.0).luminosity;
        assert!((luminosity - MAX_LUMINOSITY).abs() < 1e-12, "{luminosity}");
        const { assert!(MAX_LUMINOSITY_AGE < MAX_AGE) };
        // The former numbering (Gyr before present) gives the same age bit for bit.
        for (epoch, before_present) in [(0.5, 3.5), (0.0, 4.0), (PRESENT_EPOCH, 0.0), (2.0, 2.0)] {
            assert_eq!(
                age_of_run(epoch, 250.0).to_bits(),
                (SOLAR_AGE - before_present + 250.0 / 1000.0).to_bits()
            );
        }
        // Later epochs: older suns.
        assert!(Sun::of_run(1.0, 0.0).luminosity < Sun::of_run(5.0, 0.0).luminosity);
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
        assert!(january.z < -0.3 && distance < 0.99, "{january} {distance}");
        assert!((january.length() - 1.0).abs() < 1e-12);
        let (july, _) = direction(&orbit, 1.0, Some(6.5 / 12.0));
        assert!(july.z > 0.3, "{july}");
    }
}
