//! How the planet is lit (#92): from the camera, as the view always did, or by the sun of the
//! simulation ([`crate::sun`]), with or without an ambient light that brightens the night side.
//!
//! The choice ([`Light`]) is an option of the realistic mode only (`modes::realistic`); the
//! data modes are always lit from the camera, as before #92. With the sun's light, the sun itself
//! is drawn in the background when it is in view (`sky.wgsl`); with the camera light it is not,
//! since a visible sun would contradict the lighting. The stars appear with the sun's light as
//! far as the exposure, which adapts to the scene like an eye, allows (see [`super::sky`]).
//!
//! The sun stands fixed in space: the simulated planet does not spin, so the sun's longitude
//! ([`SUN_LONGITUDE`]) is a fixed time of day, chosen so that the default camera sees a
//! three-quarter lit disc. Its declination is 0 (an equinox) in the annual view, or that of
//! mid-January or mid-July with the season option of the realistic mode ([`season_fraction`]). Its age, and with it brightness, size and colour, follows
//! the displayed time ([`crate::sun::Sun::of_run`]).
//!
//! The exposure follows the sun: the sunlit side has the same brightness at any age, only the
//! colour of the light changes (warmer for the young sun). The ambient light is artistic: the
//! night side of a real planet gets almost no light.

use glam::{DVec3, Vec3};

use crate::sun::{self, Sun};
use crate::temperature::OrbitParams;

/// Longitude of the sun in radians in the planet's frame: 45° west of the default camera
/// (which looks from longitude 0), on the side the camera light comes from.
const SUN_LONGITUDE: f64 = -std::f64::consts::FRAC_PI_4;

/// The fraction of the year after perihelion in the middle of the month the season shows (the
/// simulation's months start at perihelion, early January), or `None` for the annual view.
pub fn season_fraction(season: crate::viewer::modes::Period) -> Option<f64> {
    use crate::viewer::modes::Period;
    match season {
        Period::Annual => None,
        Period::January => Some(0.5 / 12.0),
        Period::July => Some(6.5 / 12.0),
    }
}

/// Share of the light that reaches every surface from all sides in [`Light::Camera`] (the view's
/// lighting before #92, unchanged).
const CAMERA_AMBIENT: f32 = 0.3;
/// Share of ambient light in [`Light::SunAmbient`]: enough to read the night side, clearly darker
/// than the day side.
const SUN_AMBIENT: f32 = 0.2;

/// Where the light comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    /// From the upper left of the camera, so the visible side is always lit (with 30% ambient
    /// light): the light of the data modes, and a choice in the realistic mode.
    Camera,
    /// Only the sun: a day side, a terminator and a dark night side. The default of the
    /// realistic mode.
    Sun,
    /// The sun plus an ambient light that makes the night side visible.
    SunAmbient,
}

impl Light {
    /// The choices of the option, in the order of [`Light::from_index`].
    pub const CHOICES: &'static [&'static str] = &["Sun", "Sun + ambient", "Camera"];
    pub const LABEL: &'static str = "Light";
    pub const HOVER: &'static str = "Where the light comes from: the sun of the simulation \
         (fixed in space; drag round to see the night side), the sun plus an ambient light that \
         keeps the night side visible, or the camera (the visible side is always lit). With the \
         sun's light, stars show where an eye would see them: on the night side, with the \
         sun hidden or out of view, fading in over a few seconds as the eye adapts to the dark";

    #[cfg(test)]
    pub fn index(self) -> usize {
        match self {
            Light::Sun => 0,
            Light::SunAmbient => 1,
            Light::Camera => 2,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Light::SunAmbient,
            2 => Light::Camera,
            _ => Light::Sun,
        }
    }
}

/// The sun as the view draws it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunView {
    pub sun: Sun,
    /// Unit vector from the planet towards the sun.
    pub direction: Vec3,
    /// Angular radius of the disc in radians (true size, about 0.27°).
    pub angular_radius: f32,
    /// Colour of the sunlight as an sRGB factor with the luminance of white (slightly above 1 in
    /// red for a black body cooler than the D65 white).
    pub light_color: [f32; 3],
    /// Colour of the disc in sRGB, the brightest channel 1.
    pub disc_color: [f32; 3],
}

impl SunView {
    /// The sun of a run that started `epoch` Gyr before present, `time` Myr into it, at
    /// `season` (the fraction of the year after perihelion; `None` for the annual view, an
    /// equinox at the mean distance).
    pub fn new(epoch: f64, time: f64, season: Option<f64>) -> Self {
        let sun = Sun::of_run(epoch, time);
        // The orbit of the simulation (`--set` cannot change it).
        let (direction, distance): (DVec3, f64) =
            sun::direction(&OrbitParams::default(), SUN_LONGITUDE, season);
        let color = sun.color().map(|channel| channel as f32);
        let brightest = color.iter().copied().fold(f32::MIN_POSITIVE, f32::max);
        Self {
            sun,
            direction: direction.as_vec3(),
            angular_radius: sun.angular_radius(distance) as f32,
            light_color: color.map(srgb),
            disc_color: color.map(|channel| srgb(channel / brightest)),
        }
    }

    /// A short description for the light option: age, brightness and temperature of the sun.
    pub fn describe(&self) -> String {
        format!(
            "Sun: {:.2} Gyr old, {:.0}% of today's brightness, {:.0} K",
            self.sun.age,
            100.0 * self.sun.luminosity,
            self.sun.effective_temperature
        )
    }
}

/// The light of the planet's shading: direction, colour (sRGB factor) and ambient share.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    pub direction: Vec3,
    pub color: [f32; 3],
    pub ambient: f32,
}

impl Lighting {
    /// The lighting of `light` for a camera at `eye` (relative to the planet's centre, +z up).
    pub fn new(light: Light, eye: Vec3, sun: &SunView) -> Self {
        match light {
            Light::Camera => {
                // From the upper left of the camera, so the visible side is always lit.
                let right = Vec3::Z.cross(eye).normalize_or(Vec3::X);
                Self {
                    direction: (eye.normalize_or(Vec3::X) + Vec3::Z * 0.5 - right * 0.6)
                        .normalize(),
                    color: [1.0; 3],
                    ambient: CAMERA_AMBIENT,
                }
            }
            Light::Sun | Light::SunAmbient => Self {
                direction: sun.direction,
                color: sun.light_color,
                ambient: if light == Light::Sun {
                    0.0
                } else {
                    SUN_AMBIENT
                },
            },
        }
    }
}

/// Encodes a linear colour channel in sRGB (values above 1 are extrapolated).
fn srgb(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        12.92 * linear.max(0.0)
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn present() -> SunView {
        SunView::new(0.0, 0.0, None)
    }

    #[test]
    fn camera_light_is_the_lighting_before_the_sun() {
        let eye = Vec3::new(4.0, 0.5, 2.0);
        let lighting = Lighting::new(Light::Camera, eye, &present());
        let right = Vec3::Z.cross(eye).normalize();
        let before = (eye.normalize() + Vec3::Z * 0.5 - right * 0.6).normalize();
        assert_eq!(lighting.direction, before);
        assert_eq!(lighting.color, [1.0; 3]);
        assert_eq!(lighting.ambient, 0.3);
    }

    #[test]
    fn sunlight_comes_from_the_sun_and_does_not_move_with_the_camera() {
        let sun = present();
        let a = Lighting::new(Light::Sun, Vec3::new(4.5, 0.0, 0.0), &sun);
        let b = Lighting::new(Light::Sun, Vec3::new(0.0, -4.5, 1.0), &sun);
        assert_eq!(a, b);
        assert_eq!(a.direction, sun.direction);
        assert_eq!(a.ambient, 0.0);
        let ambient = Lighting::new(Light::SunAmbient, Vec3::X, &sun);
        assert!(ambient.ambient > 0.0 && ambient.direction == sun.direction);
        // Over the equator, 45° west of the default camera.
        assert!(sun.direction.z.abs() < 1e-6);
        assert!((sun.direction - Vec3::new(1.0, -1.0, 0.0).normalize()).length() < 1e-6);
    }

    #[test]
    fn the_season_moves_the_sun() {
        use crate::viewer::modes::Period;
        let january = SunView::new(0.0, 0.0, season_fraction(Period::January));
        let july = SunView::new(0.0, 0.0, season_fraction(Period::July));
        // About 21° south and north; nearer to the sun in January (perihelion).
        assert!(january.direction.z < -0.3 && july.direction.z > 0.3);
        assert!(january.angular_radius > july.angular_radius);
        assert!(present().direction.z.abs() < 1e-6);
    }

    #[test]
    fn the_sun_has_a_plausible_size_and_colour() {
        let sun = present();
        assert!((sun.angular_radius.to_degrees() - 0.2665).abs() < 0.001);
        // Warm white: red ≥ green ≥ blue, all close to 1.
        let [r, g, b] = sun.light_color;
        assert!(
            r >= g && g >= b && b > 0.9 && r < 1.1,
            "{:?}",
            sun.light_color
        );
        assert!((sun.disc_color[0] - 1.0).abs() < 1e-6);
        // The young sun of the Archean is smaller and redder.
        let young = SunView::new(3.5, 0.0, None);
        assert!(young.angular_radius < sun.angular_radius);
        assert!(young.disc_color[2] < sun.disc_color[2]);
        assert!(young.describe().contains("77%"), "{}", young.describe());
    }

    #[test]
    fn choices_round_trip() {
        for (index, _) in Light::CHOICES.iter().enumerate() {
            assert_eq!(Light::from_index(index).index(), index);
        }
        assert_eq!(Light::from_index(0), Light::Sun);
    }
}
