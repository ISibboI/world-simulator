//! The sky behind the planet (#92): the sun and the stars, drawn by `sky.wgsl` with the sun's
//! light ([`super::lighting::Light::Sun`] and `SunAmbient`).
//!
//! **Exposure.** A sunlit surface is about 10⁷ times brighter than the brightest stars per pixel
//! (a sunlit landscape ~10⁴ cd/m² against a star of magnitude 0 spread over a pixel), so a
//! camera exposed for the day side records no stars, and neither does one with the sun (or its
//! glare) in the frame. Stars appear only in a long exposure of a dark scene: the night side,
//! with the sun hidden behind the planet or out of view. The view meters like a camera with an
//! automatic exposure that sets the brightest ~0.1–1% of the frame to white
//! ([`Exposure::new`]): the share of the view covered by the sunlit planet (weighted by the
//! cosine of the sun's incidence, the brightness of a Lambertian surface) darkens the scene
//! between 0.1% and 1%; the glare of the visible part of the sun does so as the sun approaches
//! the frame (from 1.5 times the view's half-width to its edge). On top of that, **no lit
//! surface may be over-exposed** (#159): the exposure is at most the one that shows the
//! brightest sunlit point in view as it looks in daylight under the sun at the zenith
//! ([`Exposure::headroom`]), however small the lit part of the frame is, so a thin crescent
//! keeps its colours (the shader caps the incidence of bump-tilted normals and keeps the limb
//! haze at its daylight brightness to match). Only the sun itself (and its glint on water) may
//! clip, as it does for the eye. The ambient fill light of `SunAmbient` is a visual aid, not light: it is not metered
//! and not brightened by the exposure. With the camera light the whole visible side is lit, so
//! there are no stars (and no sun) in that mode.
//!
//! **Adaptation.** The meter gives the exposure the scene asks for ([`Exposure::target`], in
//! stops above the daylight exposure, up to [`DARK_ADAPTED`]); the view's exposure follows it
//! like an eye (or a camera's auto-exposure) in wall-clock time ([`Adaptation`]): slowly when
//! the scene gets darker (dark adaptation, a few seconds), fast when it gets brighter (light
//! adaptation). The exposure brightens the sunlight on the planet and the sun with its glow by
//! 2^stops, and lets fainter stars through: their limiting magnitude rises by 2.5 log₁₀ 2 ≈ 0.75
//! magnitudes per stop. So after the sun disappears behind the planet the stars fade in, the
//! brightest first, and when the sunlit side comes into view the image is briefly over-exposed.
//! See `notes/issue-92/exposure.md`.
//!
//! **Stars.** A procedural sky, not Earth's: a deterministic star field fixed in space (the
//! planet does not spin), generated in the shader from a hash of cells on a cube map. Their
//! magnitudes follow the counts of the real sky, about 2.5–3 times more stars per magnitude
//! (`N(<m) ∝ 10^(0.45 m)`), down to magnitude 7 (~15 000 stars over the sky, as a long exposure
//! records them); their colours are black bodies from 3200 K (M giants) to 20 000 K (B stars),
//! weighted like the naked-eye stars, most of which are K giants and A–B stars. See
//! `notes/issue-92/stars.md`.

use glam::{Mat4, Vec3, Vec4};

use crate::sun;

/// Share of the view the sunlit planet (cosine-weighted) may cover before the metered exposure
/// starts to drop from [`DARK_ADAPTED`], and where it reaches the daylight exposure.
const STARS_FADE_START: f32 = 1e-3;
const STARS_FADE_END: f32 = 1e-2;
/// How far beyond the view the sun's glare still sets the exposure, in multiples of the view's
/// half-width (in normalised device coordinates).
const GLARE_REACH: f32 = 1.5;
/// Samples of the view in each direction for the metering (a sample is 1/6000 of the view, a
/// fifth of the share where the stars start to fade; the cosine weighting smooths the steps).
const METER_COLUMNS: usize = 96;
const METER_ROWS: usize = 64;

/// The exposure of a fully dark-adapted view, in stops (factors of 2) above the daylight
/// exposure, at which every star of the field shows. The real gap between a sunlit day side and
/// the stars is ~23 stops (10⁷); 14 are enough for the stars to vanish in daylight (see
/// [`STAR_DIMMING_PER_STOP`]) and keep the fade-in within seconds.
pub const DARK_ADAPTED: f32 = 14.0;
/// Magnitudes per stop: 2.5 log₁₀ 2.
pub const STAR_DIMMING_PER_STOP: f32 = 0.752_575;
/// Time constant of the adaptation to a darker scene (dark adaptation) in seconds.
pub const DARK_ADAPTATION_SECONDS: f32 = 2.0;
/// Time constant of the adaptation to a brighter scene (light adaptation) in seconds.
pub const LIGHT_ADAPTATION_SECONDS: f32 = 0.5;
const _: () = assert!(LIGHT_ADAPTATION_SECONDS < DARK_ADAPTATION_SECONDS / 3.0);
/// Distance from the target, in stops, below which the adaptation counts as converged and
/// snaps to it (far below a visible change).
const CONVERGED: f32 = 0.02;

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
    /// How dark the metered scene is, from 0 (exposed for daylight) to 1 (a long exposure that
    /// shows all stars).
    pub darkness: f32,
    /// The largest cosine of the sun's incidence on the visible surface (1 when the subsolar
    /// point is in view, 0 when only the night side is): the brightest lit point.
    pub brightest: f32,
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
        let glare = if clip.w > 0.0 {
            let edge = (clip.x.abs().max(clip.y.abs())) / clip.w;
            sun_visible * (1.0 - smoothstep(1.0, GLARE_REACH, edge))
        } else {
            0.0
        };
        let sunlit = sunlit_share(view_projection.inverse(), sun);
        // Each of the two sets a daylight exposure on its own; the glare over its reach.
        let darkness = (1.0 - smoothstep(STARS_FADE_START, STARS_FADE_END, sunlit)) * (1.0 - glare);
        Self {
            sun_visible,
            sun_in_view,
            sunlit,
            darkness,
            brightest: brightest_incidence(eye, sun),
        }
    }

    /// The most exposure, in stops above the daylight exposure, that keeps the brightest lit
    /// point in view from over-exposing: a reflectance `a` lit at incidence cosine `c` shows as
    /// `a c 2^stops`, at most `a` (its brightness under the zenith sun in daylight) for
    /// `stops = log₂(1/c)`. Unlimited ([`DARK_ADAPTED`]) when no lit surface is in view.
    pub fn headroom(&self) -> f32 {
        if self.brightest <= 0.0 {
            DARK_ADAPTED
        } else {
            (1.0 / self.brightest).log2().clamp(0.0, DARK_ADAPTED)
        }
    }

    /// The exposure the scene asks for, in stops above the daylight exposure (0 to
    /// [`DARK_ADAPTED`]): as dark as the metered scene asks, but never so much that the
    /// brightest lit point over-exposes.
    pub fn target(&self) -> f32 {
        (DARK_ADAPTED * self.darkness).min(self.headroom())
    }
}

/// The exposure of the view as it adapts to the scene in wall-clock time, like an eye: it moves
/// exponentially towards the metered exposure (in stops, i.e. in log luminance), with
/// [`DARK_ADAPTATION_SECONDS`] when the scene gets darker and [`LIGHT_ADAPTATION_SECONDS`] when
/// it gets brighter.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Adaptation {
    /// The current exposure in stops above the daylight exposure.
    stops: f32,
    /// The target set by the last update, which holds until the next one; `None` before the
    /// first (the view then starts adapted).
    target: Option<f32>,
}

impl Adaptation {
    /// The current exposure in stops above the daylight exposure.
    pub fn stops(&self) -> f32 {
        self.stops
    }

    /// Adapted to daylight (the exposure of the camera light): the sky's exposure starts from
    /// there when the sun's light is chosen.
    pub fn daylight() -> Self {
        Self {
            stops: 0.0,
            target: Some(0.0),
        }
    }

    /// Advances by `seconds` of wall-clock time during which the scene asked for the target of
    /// the last update (the scene of the previous frame was shown until now), then sets
    /// `target` for the time until the next update. Returns whether the exposure still moves,
    /// i.e. whether the view needs another frame.
    ///
    /// The step is the exact solution for a constant target, so any split of the time gives
    /// the same result (frame-rate independent).
    pub fn update(&mut self, target: f32, seconds: f32) -> bool {
        // A broken metering must not keep the view drawing forever.
        let target = if target.is_finite() { target } else { 0.0 };
        let held = *self.target.get_or_insert_with(|| {
            self.stops = target;
            target
        });
        let difference = held - self.stops;
        let time_constant = if difference > 0.0 {
            DARK_ADAPTATION_SECONDS
        } else {
            LIGHT_ADAPTATION_SECONDS
        };
        self.stops = held - difference * (-seconds.max(0.0) / time_constant).exp();
        if (held - self.stops).abs() < CONVERGED {
            self.stops = held;
        }
        self.target = Some(target);
        self.stops != target
    }

    /// The factor on the sunlight and the sun, whose colours are sRGB-encoded: 2^stops in
    /// linear light, so about 2^(stops / 2.2) on the sRGB values (the stars' magnitudes are
    /// linear and encoded in the shader).
    pub fn srgb_gain(&self) -> f32 {
        (self.stops() / 2.2).exp2()
    }

    /// How many magnitudes fainter the stars appear than to a fully dark-adapted view (0 when
    /// dark-adapted, ~10.5 in daylight, where none is left).
    pub fn star_dimming(&self) -> f32 {
        (DARK_ADAPTED - self.stops()) * STAR_DIMMING_PER_STOP
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

/// The largest cosine of the incidence of the sun (`sun`, a unit vector) on the part of the unit
/// sphere visible from `eye`, 0 if none of it is lit. Exact, however thin the lit crescent: the
/// visible cap around the eye direction has the angular radius `α = acos(1/|eye|)`, so the lit
/// point nearest to the subsolar point is `max(θ − α, 0)` from it, `θ` the angle between the
/// sun and the eye direction. (The whole visible cap is in the frame: the planet's angular
/// radius is ~12.8° at the camera's 4.5 radii, the view's half-angle on its narrow axis 17°;
/// where panels cover part of the planet, the cap is merely conservative.) The bump relief can
/// tilt normals further towards the sun; `realistic.wgsl` caps their incidence to match.
fn brightest_incidence(eye: Vec3, sun: Vec3) -> f32 {
    let distance = eye.length();
    if distance <= 1.0 {
        return 1.0;
    }
    let cap = (1.0 / distance).acos();
    let toward_eye = eye / distance;
    let angle = toward_eye.cross(sun).length().atan2(toward_eye.dot(sun));
    (angle - cap).max(0.0).cos().max(0.0)
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

    /// The brightest star of the field and the apparent magnitude where stars vanish on screen
    /// (`BRIGHTEST` and `VISIBLE_LIMIT` in `sky.wgsl`).
    const BRIGHTEST_STAR: f32 = -1.5;
    const VISIBLE_LIMIT: f32 = 9.0;

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
        assert_eq!(exposure.darkness, 0.0);
    }

    #[test]
    fn eclipse_shows_the_stars() {
        // The sun exactly behind the planet: only the night side in view, the sun hidden.
        let eye = Vec3::new(4.5, 0.0, 0.0);
        let exposure = Exposure::new(view_projection(eye), eye, -Vec3::X, 0.0047);
        assert_eq!(exposure.sun_visible, 0.0);
        assert!(!exposure.sun_in_view);
        assert_eq!(exposure.sunlit, 0.0);
        assert_eq!(exposure.darkness, 1.0);
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
        assert_eq!(exposure.darkness, 0.0);
        // Just out of view (30° beside it; the view reaches ~26°): its glare still dims them.
        let angle = 30f32.to_radians();
        let sun = Vec3::new(-angle.cos(), angle.sin(), 0.0);
        let exposure = Exposure::new(view_projection(eye), eye, sun, 0.0047);
        assert!(!exposure.sun_in_view);
        assert!(exposure.darkness < 0.5, "{exposure:?}");
    }

    #[test]
    fn no_lit_surface_is_over_exposed() {
        // #159: from behind the planet towards the day side, the thin crescent must never be
        // brightened beyond its daylight look at the zenith sun: the brightest lit point times
        // the exposure's gain stays at most 1.
        let sun = -Vec3::X;
        for degrees in 0..=180 {
            let angle = (degrees as f32).to_radians();
            let eye = Vec3::new(angle.cos(), angle.sin(), 0.0) * 4.5;
            let exposure = Exposure::new(view_projection(eye), eye, sun, 0.0047);
            let target = exposure.target();
            assert!(
                exposure.brightest * target.exp2() <= 1.0 + 1e-4,
                "{degrees}° {exposure:?} target {target}"
            );
        }
        // The angle of the report: a thin crescent, 40° from the eclipse. It filled less than
        // 0.2% of the frame, which asked for ~13.6 stops; now it is shown at most ~1.1 stops
        // brighter than daylight.
        let angle = 40f32.to_radians();
        let eye = Vec3::new(angle.cos(), angle.sin(), 0.0) * 4.5;
        let exposure = Exposure::new(view_projection(eye), eye, sun, 0.0047);
        assert!(exposure.darkness > 0.9, "{exposure:?}");
        assert!(exposure.target() < 1.2, "{exposure:?}");
        assert!(exposure.target() > 1.0, "{exposure:?}");
    }

    #[test]
    fn the_brightest_point_is_exact() {
        // Independent check: the largest p·sun over the visible cap {p·eye ≥ 1} lies at the
        // subsolar point if it is visible, else on the cap's boundary circle, sampled densely.
        let eye = Vec3::new(0.3, -4.0, 1.2);
        let axis = eye.normalize();
        let radius = (1.0 / eye.length()).acos();
        let (u, v) = axis.any_orthonormal_pair();
        for sun in [
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.2, 0.9, -0.3).normalize(),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(-0.1, 0.95, 0.2).normalize(),
        ] {
            let mut best: f32 = if sun.dot(eye) >= 1.0 { 1.0 } else { 0.0 };
            for step in 0..100_000 {
                let phi = std::f32::consts::TAU * step as f32 / 100_000.0;
                let p = axis * radius.cos() + (u * phi.cos() + v * phi.sin()) * radius.sin();
                best = best.max(p.dot(sun));
            }
            let exact = brightest_incidence(eye, sun);
            assert!(
                (exact - best).abs() <= 1e-3 * best.max(1e-3) + 1e-5,
                "{sun}: {exact} vs {best}"
            );
        }
    }

    #[test]
    fn eclipse_and_daylight_targets() {
        let eye = Vec3::new(4.5, 0.0, 0.0);
        // Only the night side in view: fully dark-adapted, all stars.
        let eclipse = Exposure::new(view_projection(eye), eye, -Vec3::X, 0.0047);
        assert_eq!(eclipse.brightest, 0.0);
        assert_eq!(eclipse.target(), DARK_ADAPTED);
        // The full day side: daylight.
        let day = Exposure::new(view_projection(eye), eye, Vec3::X, 0.0047);
        assert_eq!(day.brightest, 1.0);
        assert_eq!(day.target(), 0.0);
    }

    #[test]
    fn the_stars_fade_as_the_crescent_grows() {
        // The camera moves from behind the planet (eclipse) towards the day side: the sunlit
        // share grows, and the stars fade with it (the sun in or near the view hides them too).
        let sun = -Vec3::X;
        let mut previous = 0.0;
        let mut faded = false;
        for degrees in 0..=60 {
            let angle = (degrees as f32).to_radians();
            let eye = Vec3::new(angle.cos(), angle.sin(), 0.0) * 4.5;
            let exposure = Exposure::new(view_projection(eye), eye, sun, 0.0047);
            assert!(exposure.sunlit >= previous, "{degrees}° {exposure:?}");
            previous = exposure.sunlit;
            // The glare of the sun in or near the view may hide them further.
            let expected = 1.0 - smoothstep(STARS_FADE_START, STARS_FADE_END, exposure.sunlit);
            assert!(exposure.darkness <= expected);
            if exposure.sun_in_view && exposure.sun_visible == 1.0 {
                assert_eq!(exposure.darkness, 0.0);
            }
            faded |= exposure.darkness > 0.0 && exposure.darkness < 1.0;
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

    /// Adapts from `from` to `to` (stops) in `steps` equal steps over `seconds`.
    fn adapt(from: f32, to: f32, seconds: f32, steps: usize) -> Adaptation {
        let mut adaptation = Adaptation::default();
        adaptation.update(from, 0.0);
        adaptation.update(to, 0.0);
        for _ in 0..steps {
            adaptation.update(to, seconds / steps as f32);
        }
        adaptation
    }

    #[test]
    fn the_view_starts_adapted_and_stays_still() {
        let mut adaptation = Adaptation::default();
        assert!(!adaptation.update(DARK_ADAPTED, 0.016));
        assert_eq!(adaptation.stops(), DARK_ADAPTED);
        assert!(!adaptation.update(DARK_ADAPTED, 10.0));
        assert_eq!(adaptation.stops(), DARK_ADAPTED);
    }

    #[test]
    fn dark_adaptation_is_slow_and_light_adaptation_fast() {
        // After one time constant, 1 − 1/e of the way.
        let dark = adapt(0.0, DARK_ADAPTED, DARK_ADAPTATION_SECONDS, 100);
        let expected = DARK_ADAPTED * (1.0 - (-1f32).exp());
        assert!((dark.stops() - expected).abs() < 1e-3, "{dark:?}");
        let light = adapt(DARK_ADAPTED, 0.0, LIGHT_ADAPTATION_SECONDS, 100);
        assert!((light.stops() - DARK_ADAPTED * (-1f32).exp()).abs() < 1e-3);
        // In one second: brightening is nearly done, darkening has far to go.
        let dark = adapt(0.0, DARK_ADAPTED, 1.0, 60);
        let light = adapt(DARK_ADAPTED, 0.0, 1.0, 60);
        assert!(light.stops() < 2.0, "{light:?}");
        assert!(dark.stops() < 0.5 * DARK_ADAPTED, "{dark:?}");
    }

    #[test]
    fn adaptation_does_not_depend_on_the_frame_rate() {
        for (from, to) in [(0.0, DARK_ADAPTED), (DARK_ADAPTED, 2.0)] {
            let reference = adapt(from, to, 0.5, 1).stops();
            for steps in [2, 7, 30, 144] {
                let stops = adapt(from, to, 0.5, steps).stops();
                assert!(
                    (stops - reference).abs() < 1e-3,
                    "{steps}: {stops} {reference}"
                );
            }
        }
    }

    #[test]
    fn a_new_target_applies_from_the_next_frame() {
        // A long idle pause, then the scene changes: the pause belongs to the old scene, so the
        // change is not skipped.
        let mut adaptation = Adaptation::default();
        adaptation.update(DARK_ADAPTED, 0.0);
        assert!(adaptation.update(0.0, 120.0));
        assert_eq!(adaptation.stops(), DARK_ADAPTED);
        adaptation.update(0.0, 0.1);
        assert!(adaptation.stops() > 0.5 * DARK_ADAPTED, "{adaptation:?}");
    }

    #[test]
    fn adaptation_converges_and_stops() {
        let mut adaptation = Adaptation::daylight();
        let mut frames = 0;
        while adaptation.update(DARK_ADAPTED, 1.0 / 60.0) {
            frames += 1;
            assert!(frames < 60 * 30, "{adaptation:?}");
        }
        assert_eq!(adaptation.stops(), DARK_ADAPTED);
        // About 2 s × ln(14 / 0.02) ≈ 13 s.
        let seconds = frames as f32 / 60.0;
        assert!((10.0..16.0).contains(&seconds), "{seconds}");
        // Light adaptation converges within about 3.5 s (0.5 s × ln(700)).
        let mut frames = 0;
        while adaptation.update(0.0, 1.0 / 60.0) {
            frames += 1;
        }
        assert!(frames < 4 * 60, "{frames}");
        assert_eq!(adaptation.stops(), 0.0);
    }

    #[test]
    fn stars_need_dark_adaptation() {
        // In daylight even the brightest star (−1.5) lies beyond the visible limit (9, see
        // `sky.wgsl`); dark-adapted, all of them (to magnitude 7) show.
        let daylight = Adaptation::daylight();
        assert!(
            BRIGHTEST_STAR + daylight.star_dimming() > VISIBLE_LIMIT,
            "{daylight:?}"
        );
        assert_eq!(daylight.srgb_gain(), 1.0);
        let mut dark = Adaptation::default();
        dark.update(DARK_ADAPTED, 0.0);
        assert_eq!(dark.star_dimming(), 0.0);
        // The limiting magnitude rises while the view adapts.
        let limit =
            |seconds: f32| VISIBLE_LIMIT - adapt(0.0, DARK_ADAPTED, seconds, 10).star_dimming();
        assert!(limit(1.0) < limit(3.0) && limit(3.0) < limit(6.0));
    }

    #[test]
    fn the_shader_has_the_same_star_limits() {
        let shader = include_str!("sky.wgsl");
        for line in [
            format!("const BRIGHTEST: f32 = {BRIGHTEST_STAR:?};"),
            format!("const VISIBLE_LIMIT: f32 = {VISIBLE_LIMIT:?};"),
        ] {
            assert!(shader.contains(&line), "{line}");
        }
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
