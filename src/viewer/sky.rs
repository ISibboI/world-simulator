//! The sky behind the planet (#92): the sun and the stars, drawn by `sky.wgsl` with the sun's
//! light ([`super::lighting::Light::Sun`] and `SunAmbient`).
//!
//! **Exposure.** A sunlit surface is about 10⁷ times brighter than the brightest stars per pixel
//! (a sunlit landscape ~10⁴ cd/m² against a star of magnitude 0 spread over a pixel), so a
//! camera exposed for the day side records no stars, and neither does one with the sun (or its
//! glare) in the frame. Stars appear only in a long exposure of a dark scene: the night side,
//! with the sun hidden behind the planet or out of view. The view meters like a camera with an
//! automatic exposure that sets the brightest ~0.1–1% of the frame to white
//! ([`Exposure::new`]): it measures the share of the view covered by the sunlit planet
//! (weighted by the cosine of the sun's incidence, the brightness of a Lambertian surface) and
//! whether the visible sun is in or near the view (its glare). Below 0.1% the stars are fully visible, above 1% they
//! are gone, and in between they fade, so they fade smoothly while the terminator or a thin
//! crescent comes into view. The ambient fill light of `SunAmbient` is a visual aid, not light,
//! and is not metered. With the camera light the whole visible side is lit, so there are no
//! stars (and no sun) in that mode.
//!
//! **Stars.** A procedural sky, not Earth's: a deterministic star field fixed in space (the
//! planet does not spin), generated in the shader from a hash of cells on a cube map. Their
//! magnitudes follow the counts of the real sky, about 2.5–3 times more stars per magnitude
//! (`N(<m) ∝ 10^(0.45 m)`), down to magnitude 7 (~15 000 stars over the sky, as a long exposure
//! records them); their colours are black bodies from 3200 K (M giants) to 20 000 K (B stars),
//! weighted like the naked-eye stars, most of which are K giants and A–F stars. See
//! `notes/issue-92/stars.md`.

use glam::{Mat4, Vec3, Vec4};

use crate::sun;

/// Share of the view the sunlit planet (cosine-weighted) may cover before the stars start to
/// fade, and where they are gone.
const STARS_FADE_START: f32 = 1e-3;
const STARS_FADE_END: f32 = 1e-2;
/// How far beyond the view the sun's glare still sets the exposure, in multiples of the view's
/// half-width (in normalised device coordinates).
const GLARE_REACH: f32 = 1.5;
/// Samples of the view in each direction for the metering (a sample is 1/30 000 of the view).
const METER_COLUMNS: usize = 200;
const METER_ROWS: usize = 150;

/// Temperatures of the star colours in K, the colours [`star_colors`] gives in this order
/// (`sky.wgsl` picks one per star with its own weights).
pub const STAR_TEMPERATURES: [f64; 8] = [
    3200.0, 4000.0, 4800.0, 5600.0, 6500.0, 8000.0, 11_000.0, 20_000.0,
];

/// The colours of [`STAR_TEMPERATURES`]: black bodies in sRGB, the brightest channel 1.
pub fn star_colors() -> [[f32; 4]; 8] {
    STAR_TEMPERATURES.map(|temperature| {
        let color = sun::black_body_color(temperature);
        let brightest = color.iter().copied().fold(f64::MIN_POSITIVE, f64::max);
        let [r, g, b] = color.map(|channel| srgb((channel / brightest) as f32));
        [r, g, b, 1.0]
    })
}

/// What the camera's exposure makes of the sky.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exposure {
    /// The share of the sun's disc that the planet does not hide (0 to 1).
    pub sun_visible: f32,
    /// Whether the visible part of the sun is in the view.
    pub sun_in_view: bool,
    /// The share of the view covered by the sunlit planet, weighted by the cosine of the
    /// incidence (1 for a view full of surface under the sun at the zenith).
    pub sunlit: f32,
    /// How visible the stars are, from 0 (exposed for daylight) to 1 (a long exposure).
    pub stars: f32,
}

impl Exposure {
    /// The exposure of a view with `view_projection` from `eye` (planet radii from the centre),
    /// with the sun in `sun` (unit vector) of `sun_radius` (radians). The planet is metered as
    /// a sphere of radius 1 (the sea surface; the exaggerated relief adds a few per cent at the
    /// limb).
    pub fn new(view_projection: Mat4, eye: Vec3, sun: Vec3, sun_radius: f32) -> Self {
        let sun_visible = sun_visibility(eye, sun, sun_radius);
        let clip = view_projection * sun.extend(0.0);
        let sun_in_view =
            sun_visible > 0.0 && clip.w > 0.0 && clip.x.abs() <= clip.w && clip.y.abs() <= clip.w;
        // Glare of the sun in or near the view (lens flare and veiling glare reach well beyond
        // the frame): full inside it, fading out to 1.5 times its half-width.
        let glare = if sun_visible > 0.0 && clip.w > 0.0 {
            let edge = (clip.x.abs().max(clip.y.abs())) / clip.w;
            1.0 - smoothstep(1.0, GLARE_REACH, edge)
        } else {
            0.0
        };
        let sunlit = sunlit_share(view_projection.inverse(), sun);
        let metered = sunlit.max(glare);
        let stars = 1.0 - smoothstep(STARS_FADE_START, STARS_FADE_END, metered);
        Self {
            sun_visible,
            sun_in_view,
            sunlit,
            stars,
        }
    }
}

/// The share of the sun's disc (angular radius `sun_radius`) that the unit sphere does not hide
/// from `eye`: linear in the overlap of the two discs, which is close for a disc much smaller
/// than the planet.
fn sun_visibility(eye: Vec3, sun: Vec3, sun_radius: f32) -> f32 {
    let distance = eye.length();
    if distance <= 1.0 {
        return 1.0;
    }
    let planet_radius = (1.0 / distance).asin();
    let to_centre = -eye / distance;
    let separation = to_centre.cross(sun).length().atan2(to_centre.dot(sun));
    ((separation - (planet_radius - sun_radius)) / (2.0 * sun_radius.max(1e-6))).clamp(0.0, 1.0)
}

/// The share of the view covered by the sunlit unit sphere, weighted by the cosine of the
/// incidence, sampled on a regular grid of the view.
fn sunlit_share(inverse_view_projection: Mat4, sun: Vec3) -> f32 {
    let unproject = |x: f32, y: f32, z: f32| {
        let point: Vec4 = inverse_view_projection * Vec4::new(x, y, z, 1.0);
        point.truncate() / point.w
    };
    let mut sum = 0.0;
    for row in 0..METER_ROWS {
        let y = 1.0 - 2.0 * (row as f32 + 0.5) / METER_ROWS as f32;
        for column in 0..METER_COLUMNS {
            let x = 2.0 * (column as f32 + 0.5) / METER_COLUMNS as f32 - 1.0;
            let origin = unproject(x, y, 0.0);
            let direction = (unproject(x, y, 1.0) - origin).normalize_or_zero();
            // The nearer intersection with the unit sphere, if any.
            let b = origin.dot(direction);
            let discriminant = b * b - (origin.length_squared() - 1.0);
            if discriminant < 0.0 {
                continue;
            }
            let t = -b - discriminant.sqrt();
            if t < 0.0 {
                continue;
            }
            let normal = (origin + direction * t).normalize_or_zero();
            sum += normal.dot(sun).max(0.0);
        }
    }
    sum / (METER_ROWS * METER_COLUMNS) as f32
}

fn smoothstep(start: f32, end: f32, x: f32) -> f32 {
    let t = ((x - start) / (end - start)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Encodes a linear colour channel in sRGB.
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

    /// A view like the 3D view's from `eye` towards the centre.
    fn view_projection(eye: Vec3) -> Mat4 {
        let view = glam::camera::rh::view::look_at_mat4(eye, Vec3::ZERO, Vec3::Z);
        let projection = glam::camera::rh::proj::directx::perspective(0.6, 1.6, 0.1, 10.0);
        projection * view
    }

    #[test]
    fn day_side_hides_the_stars() {
        let eye = Vec3::new(4.5, 0.0, 0.0);
        // The sun behind the camera: a full disc in daylight.
        let exposure = Exposure::new(view_projection(eye), eye, Vec3::X, 0.0047);
        assert!(!exposure.sun_in_view);
        // The disc covers about a fifth of a 16:10 view, its mean cosine is 2/3.
        assert!(exposure.sunlit > 0.05, "{exposure:?}");
        assert_eq!(exposure.stars, 0.0);
    }

    #[test]
    fn eclipse_shows_the_stars() {
        // The sun exactly behind the planet: only the night side in view, the sun hidden.
        let eye = Vec3::new(4.5, 0.0, 0.0);
        let exposure = Exposure::new(view_projection(eye), eye, -Vec3::X, 0.0047);
        assert_eq!(exposure.sun_visible, 0.0);
        assert!(!exposure.sun_in_view);
        assert_eq!(exposure.sunlit, 0.0);
        assert_eq!(exposure.stars, 1.0);
    }

    #[test]
    fn the_visible_sun_hides_the_stars() {
        // The sun 18° beside the view direction, beside the planet (12.8°), inside the view
        // (~26° horizontally).
        let eye = Vec3::new(4.5, 0.0, 0.0);
        let angle = 18f32.to_radians();
        let sun = Vec3::new(-angle.cos(), angle.sin(), 0.0);
        let exposure = Exposure::new(view_projection(eye), eye, sun, 0.0047);
        assert_eq!(exposure.sun_visible, 1.0);
        assert!(exposure.sun_in_view);
        assert_eq!(exposure.stars, 0.0);
        // Just out of view (30° beside it; the view reaches ~26°): its glare still hides them.
        let angle = 30f32.to_radians();
        let sun = Vec3::new(-angle.cos(), angle.sin(), 0.0);
        let exposure = Exposure::new(view_projection(eye), eye, sun, 0.0047);
        assert!(!exposure.sun_in_view);
        assert_eq!(exposure.stars, 0.0);
    }

    #[test]
    fn the_stars_fade_as_the_crescent_grows() {
        // The camera moves from behind the planet (eclipse) towards the day side: the sunlit
        // share grows, and the stars fade with it (except while the sun itself is in view,
        // which hides them anyway).
        let sun = -Vec3::X;
        let mut previous = 0.0;
        let mut faded = false;
        for degrees in 0..=60 {
            let angle = (degrees as f32).to_radians();
            let eye = Vec3::new(angle.cos(), angle.sin(), 0.0) * 4.5;
            let exposure = Exposure::new(view_projection(eye), eye, sun, 0.0047);
            assert!(exposure.sunlit >= previous, "{degrees}° {exposure:?}");
            previous = exposure.sunlit;
            if exposure.sun_in_view {
                assert_eq!(exposure.stars, 0.0);
            } else {
                // The glare of the sun just out of view may hide them further.
                let expected = 1.0 - smoothstep(STARS_FADE_START, STARS_FADE_END, exposure.sunlit);
                assert!(exposure.stars <= expected);
                faded |= exposure.stars > 0.0 && exposure.stars < 1.0;
            }
        }
        assert!(faded, "no partial fade");
        assert!(previous > STARS_FADE_END);
    }

    #[test]
    fn the_sun_disappears_behind_the_limb_gradually() {
        let eye = Vec3::new(4.5, 0.0, 0.0);
        let planet = (1.0f32 / 4.5).asin();
        let radius = 0.0047;
        let at = |separation: f32| {
            sun_visibility(
                eye,
                Vec3::new(-separation.cos(), separation.sin(), 0.0),
                radius,
            )
        };
        assert_eq!(at(planet - 2.0 * radius), 0.0);
        assert!((at(planet) - 0.5).abs() < 1e-3);
        assert_eq!(at(planet + 2.0 * radius), 1.0);
    }

    #[test]
    fn star_colours_run_from_red_to_blue() {
        let colors = star_colors();
        let ratio = |c: [f32; 4]| c[2] / c[0];
        for pair in colors.windows(2) {
            assert!(ratio(pair[1]) > ratio(pair[0]), "{colors:?}");
        }
        assert!(
            ratio(colors[0]) < 0.6 && ratio(colors[7]) > 1.2,
            "{colors:?}"
        );
    }
}
