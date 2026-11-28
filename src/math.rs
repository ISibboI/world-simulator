//! The transcendental functions of the simulation (`sin`, `exp`, `ln`, `atan2`, `powf`, …), all
//! in one place, so that a desktop run can use exactly the implementations of the web version.
//!
//! The basic operations (`+ − × ÷`, `sqrt`, `mul_add`, rounding, conversions) are exactly rounded
//! by IEEE 754 and give the same bits on every platform. The transcendental functions are not:
//! each platform's math library rounds some results differently in the last bit (glibc on Linux,
//! the C runtime on Windows and macOS), and the simulation is chaotic, so a last-bit difference
//! grows into a different planet. The web version (wasm32) has no system math library; Rust
//! takes these functions from its own port of musl's libm there, and this module calls the same
//! code, the [`libm`] crate, directly.
//!
//! - By default, natively, the functions are the platform's (`f64::sin` etc.), as fast and with
//!   the same results as without this module.
//! - With `--web-math` ([`set_web_math`]), and always on the web, they are the [`libm`] crate's,
//!   which is plain Rust built from the basic operations: a desktop run gives bit for bit the
//!   same planet as the web version with the same parameters, on every platform.
//!
//! The simulation calls these functions instead of the methods of `f64` and of glam's
//! `DVec3`/`DQuat` that use them; Clippy's `disallowed_methods` (see `clippy.toml`) rejects the
//! methods elsewhere. Only the 3D view (`viewer`), which does not change the simulation, uses
//! them directly. Everything else in this crate goes through here, also the snapshot images.
//! The simulation computes in `f64` only; `f32` (used by the 3D view) is not covered.
//! `powi` needs no switch: it is a sequence of multiplications, the same everywhere.

// This is the one place that calls the platform's functions.
#![allow(clippy::disallowed_methods)]
// The full set of the transcendental functions of `f64`, so that new code finds them here; not
// all of them are used yet.
#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};

use glam::{DQuat, DVec3};

/// Whether the functions are the web version's ([`portable`]) rather than the platform's.
static WEB_MATH: AtomicBool = AtomicBool::new(false);

/// Selects the web version's implementations (`true`) or the platform's (`false`, the default)
/// for all following calls, on all threads. Has no effect on the web, which always uses its own.
/// Set it before the simulation starts; switching while one runs mixes the two.
pub fn set_web_math(on: bool) {
    WEB_MATH.store(on, Ordering::Relaxed);
}

/// Whether the functions are the web version's: always on the web, natively if selected.
#[inline(always)]
pub fn web_math() -> bool {
    cfg!(target_arch = "wasm32") || WEB_MATH.load(Ordering::Relaxed)
}

/// The platform's implementations: Rust's standard library, which calls the system math library
/// (or LLVM intrinsics that do). The default natively.
pub mod platform {
    #![allow(clippy::disallowed_methods)]

    macro_rules! unary {
        ($($name:ident),* $(,)?) => {$(
            #[inline(always)]
            pub fn $name(x: f64) -> f64 {
                x.$name()
            }
        )*};
    }
    macro_rules! binary {
        ($($name:ident),* $(,)?) => {$(
            #[inline(always)]
            pub fn $name(x: f64, y: f64) -> f64 {
                x.$name(y)
            }
        )*};
    }
    unary!(
        sin, cos, tan, asin, acos, atan, sinh, cosh, tanh, asinh, acosh, atanh, exp, exp2, exp_m1,
        ln, ln_1p, log2, log10, cbrt,
    );
    binary!(atan2, powf, hypot);
}

/// The web version's implementations: the [`libm`] crate (the code that Rust's standard library
/// uses on `wasm32-unknown-unknown`, built from the basic operations only, so it gives the same
/// bits everywhere). Always used on the web, and natively with `--web-math`.
pub mod portable {
    macro_rules! unary {
        ($($name:ident => $libm:ident),* $(,)?) => {$(
            #[inline(always)]
            pub fn $name(x: f64) -> f64 {
                libm::$libm(x)
            }
        )*};
    }
    macro_rules! binary {
        ($($name:ident => $libm:ident),* $(,)?) => {$(
            #[inline(always)]
            pub fn $name(x: f64, y: f64) -> f64 {
                libm::$libm(x, y)
            }
        )*};
    }
    unary!(
        sin => sin, cos => cos, tan => tan, asin => asin, acos => acos, atan => atan,
        sinh => sinh, cosh => cosh, tanh => tanh, asinh => asinh, acosh => acosh,
        atanh => atanh, exp => exp, exp2 => exp2, exp_m1 => expm1, ln => log, ln_1p => log1p,
        log2 => log2, log10 => log10, cbrt => cbrt,
    );
    binary!(atan2 => atan2, powf => pow, hypot => hypot);
}

macro_rules! dispatch_unary {
    ($($(#[$doc:meta])* $name:ident;)*) => {$(
        $(#[$doc])*
        #[inline]
        pub fn $name(x: f64) -> f64 {
            if web_math() { portable::$name(x) } else { platform::$name(x) }
        }
    )*};
}
macro_rules! dispatch_binary {
    ($($(#[$doc:meta])* $name:ident;)*) => {$(
        $(#[$doc])*
        #[inline]
        pub fn $name(x: f64, y: f64) -> f64 {
            if web_math() { portable::$name(x, y) } else { platform::$name(x, y) }
        }
    )*};
}

dispatch_unary! {
    /// Sine ([`f64::sin`]).
    sin;
    /// Cosine ([`f64::cos`]).
    cos;
    /// Tangent ([`f64::tan`]).
    tan;
    /// Arcsine ([`f64::asin`]).
    asin;
    /// Arccosine ([`f64::acos`]).
    acos;
    /// Arctangent ([`f64::atan`]).
    atan;
    /// Hyperbolic sine ([`f64::sinh`]).
    sinh;
    /// Hyperbolic cosine ([`f64::cosh`]).
    cosh;
    /// Hyperbolic tangent ([`f64::tanh`]).
    tanh;
    /// Inverse hyperbolic sine ([`f64::asinh`]).
    asinh;
    /// Inverse hyperbolic cosine ([`f64::acosh`]).
    acosh;
    /// Inverse hyperbolic tangent ([`f64::atanh`]).
    atanh;
    /// `e^x` ([`f64::exp`]).
    exp;
    /// `2^x` ([`f64::exp2`]).
    exp2;
    /// `e^x - 1` ([`f64::exp_m1`]).
    exp_m1;
    /// Natural logarithm ([`f64::ln`]).
    ln;
    /// `ln(1 + x)` ([`f64::ln_1p`]).
    ln_1p;
    /// Base-2 logarithm ([`f64::log2`]).
    log2;
    /// Base-10 logarithm ([`f64::log10`]).
    log10;
    /// Cube root ([`f64::cbrt`]).
    cbrt;
}

dispatch_binary! {
    /// Four-quadrant arctangent of `y / x` ([`f64::atan2`]).
    atan2;
    /// `x^y` ([`f64::powf`]).
    powf;
    /// `sqrt(x² + y²)` without overflow ([`f64::hypot`]).
    hypot;
}

/// Sine and cosine ([`f64::sin_cos`], which computes them separately on every platform).
#[inline]
pub fn sin_cos(x: f64) -> (f64, f64) {
    (sin(x), cos(x))
}

/// The complementary error function `erfc(x) = 1 − erf(x)`, from [`libm`] on every platform
/// (the standard library has no stable `erfc`), so it gives the same bits everywhere.
#[inline]
pub fn erfc(x: f64) -> f64 {
    libm::erfc(x)
}

/// The angle in radians between two non-zero vectors, as [`DVec3::angle_between`] computes it.
#[inline]
pub fn angle_between(a: DVec3, b: DVec3) -> f64 {
    acos((a.dot(b) / (a.length_squared() * b.length_squared()).sqrt()).clamp(-1.0, 1.0))
}

/// The rotation by `angle` radians around the unit vector `axis`, as
/// [`DQuat::from_axis_angle`] computes it.
#[inline]
pub fn quat_from_axis_angle(axis: DVec3, angle: f64) -> DQuat {
    let (s, c) = sin_cos(angle * 0.5);
    let v = axis * s;
    DQuat::from_xyzw(v.x, v.y, v.z, c)
}

/// The rotation by `v.length()` radians around `v`, as [`DQuat::from_scaled_axis`] computes it.
#[inline]
pub fn quat_from_scaled_axis(v: DVec3) -> DQuat {
    let length = v.length();
    if length == 0.0 {
        DQuat::IDENTITY
    } else {
        quat_from_axis_angle(v / length, length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inputs over the ranges the simulation uses and beyond: angles, exponents, ratios, and a
    /// few special values.
    fn inputs() -> Vec<f64> {
        let mut inputs = vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.5,
            1e-300,
            1e-12,
            std::f64::consts::PI,
            std::f64::consts::FRAC_PI_2,
            700.0,
            -745.0,
            1e10,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];
        // A deterministic spread of values from -40 to 40 with arbitrary low bits.
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for _ in 0..2000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let unit = (state >> 11) as f64 / (1u64 << 53) as f64;
            inputs.push(80.0 * unit - 40.0);
            inputs.push((unit - 0.5) * 2.0);
        }
        inputs
    }

    fn same(a: f64, b: f64) -> bool {
        a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
    }

    /// Whether `a` and `b` are within `ulps` units in the last place (or both NaN).
    fn close(a: f64, b: f64, ulps: u64) -> bool {
        if a.is_nan() || b.is_nan() {
            return a.is_nan() && b.is_nan();
        }
        if a == b {
            return true;
        }
        if a.is_sign_negative() != b.is_sign_negative() || a.is_infinite() || b.is_infinite() {
            return false;
        }
        a.to_bits().abs_diff(b.to_bits()) <= ulps
    }

    type Function = fn(f64) -> f64;
    type Unary = (&'static str, Function, Function, Function);

    fn unary_cases() -> Vec<Unary> {
        vec![
            ("sin", platform::sin, portable::sin, |x| x.sin()),
            ("cos", platform::cos, portable::cos, |x| x.cos()),
            ("tan", platform::tan, portable::tan, |x| x.tan()),
            ("asin", platform::asin, portable::asin, |x| x.asin()),
            ("acos", platform::acos, portable::acos, |x| x.acos()),
            ("atan", platform::atan, portable::atan, |x| x.atan()),
            ("sinh", platform::sinh, portable::sinh, |x| x.sinh()),
            ("cosh", platform::cosh, portable::cosh, |x| x.cosh()),
            ("tanh", platform::tanh, portable::tanh, |x| x.tanh()),
            ("asinh", platform::asinh, portable::asinh, |x| x.asinh()),
            ("acosh", platform::acosh, portable::acosh, |x| x.acosh()),
            ("atanh", platform::atanh, portable::atanh, |x| x.atanh()),
            ("exp", platform::exp, portable::exp, |x| x.exp()),
            ("exp2", platform::exp2, portable::exp2, |x| x.exp2()),
            ("exp_m1", platform::exp_m1, portable::exp_m1, |x| x.exp_m1()),
            ("ln", platform::ln, portable::ln, |x| x.ln()),
            ("ln_1p", platform::ln_1p, portable::ln_1p, |x| x.ln_1p()),
            ("log2", platform::log2, portable::log2, |x| x.log2()),
            ("log10", platform::log10, portable::log10, |x| x.log10()),
            ("cbrt", platform::cbrt, portable::cbrt, |x| x.cbrt()),
        ]
    }

    /// The portable functions are the `libm` crate's, bit for bit.
    #[test]
    fn portable_is_libm() {
        let libm: [(&str, Function); 20] = [
            ("sin", libm::sin),
            ("cos", libm::cos),
            ("tan", libm::tan),
            ("asin", libm::asin),
            ("acos", libm::acos),
            ("atan", libm::atan),
            ("sinh", libm::sinh),
            ("cosh", libm::cosh),
            ("tanh", libm::tanh),
            ("asinh", libm::asinh),
            ("acosh", libm::acosh),
            ("atanh", libm::atanh),
            ("exp", libm::exp),
            ("exp2", libm::exp2),
            ("exp_m1", libm::expm1),
            ("ln", libm::log),
            ("ln_1p", libm::log1p),
            ("log2", libm::log2),
            ("log10", libm::log10),
            ("cbrt", libm::cbrt),
        ];
        for ((name, _, portable, _), (libm_name, libm)) in unary_cases().into_iter().zip(libm) {
            assert_eq!(name, libm_name);
            for x in inputs() {
                assert!(same(portable(x), libm(x)), "{name}({x:e})");
            }
        }
        for x in inputs() {
            for y in [-2.5, -1.0, 0.0, 0.3, 1.0, 1.7, 3.0, x] {
                assert!(same(portable::atan2(x, y), libm::atan2(x, y)));
                assert!(same(portable::powf(x, y), libm::pow(x, y)));
                assert!(same(portable::hypot(x, y), libm::hypot(x, y)));
            }
        }
    }

    /// The platform functions are the standard library's, bit for bit, and so is the default
    /// dispatch (tests never select the web version's functions, which would affect the other
    /// tests running at the same time).
    #[test]
    fn platform_and_default_are_std() {
        assert!(cfg!(target_arch = "wasm32") || !web_math());
        let dispatched: [fn(f64) -> f64; 20] = [
            sin, cos, tan, asin, acos, atan, sinh, cosh, tanh, asinh, acosh, atanh, exp, exp2,
            exp_m1, ln, ln_1p, log2, log10, cbrt,
        ];
        for ((name, platform, _, std), dispatched) in unary_cases().into_iter().zip(dispatched) {
            for x in inputs() {
                assert!(same(platform(x), std(x)), "{name}({x:e})");
                assert!(same(dispatched(x), std(x)), "{name}({x:e})");
            }
        }
        for x in inputs() {
            for y in [-2.5, -1.0, 0.0, 0.3, 1.0, 1.7, 3.0, x] {
                assert!(same(atan2(x, y), x.atan2(y)));
                assert!(same(powf(x, y), x.powf(y)));
                assert!(same(hypot(x, y), x.hypot(y)));
            }
            let (s, c) = sin_cos(x);
            let (std_s, std_c) = x.sin_cos();
            assert!(same(s, std_s) && same(c, std_c));
        }
    }

    /// Both implementations are accurate: they agree to about 1e-13 (most within one unit in
    /// the last place; `atanh` near ±1 differs most), which also catches a function wired to the
    /// wrong implementation.
    #[test]
    fn implementations_agree_closely() {
        for (name, platform, portable, _) in unary_cases() {
            for x in inputs() {
                assert!(
                    close(platform(x), portable(x), 1024),
                    "{name}({x:e}): {} vs {}",
                    platform(x),
                    portable(x)
                );
            }
        }
        for x in inputs() {
            for y in [-2.5, -1.0, 0.3, 1.0, 1.7, 3.0] {
                assert!(close(platform::atan2(x, y), portable::atan2(x, y), 4));
                assert!(close(platform::powf(x, y), portable::powf(x, y), 4));
            }
        }
    }

    /// Results of the web version's functions, recorded with `libm` 0.2.16 (the version in
    /// Rust's standard library for wasm32 as of Rust 1.100). If these change, e.g. after updating
    /// `libm`, seeds give other planets on the web and with `--web-math` than before.
    #[test]
    fn portable_results_are_unchanged() {
        let cases: [(f64, u64); 8] = [
            (portable::sin(1.0), 0x3fea_ed54_8f09_0cee),
            (portable::cos(2.5), 0xbfe9_a2f7_ef85_8b7d),
            (portable::tan(0.7), 0x3fea_f406_c2fc_78ae),
            (portable::asin(0.3), 0x3fd3_8015_9e14_f6ff),
            (portable::atan2(0.4, -1.3), 0x4006_bea7_ebdb_bf5c),
            (portable::exp(-3.7), 0x3f99_511f_c687_1044),
            (portable::ln(17.0), 0x4006_aa6b_c1fa_7f7a),
            (portable::powf(2.7, 1.3), 0x400d_1915_63de_7939),
        ];
        for (index, (result, bits)) in cases.into_iter().enumerate() {
            assert_eq!(result.to_bits(), bits, "case {index}: {result:e}");
        }
    }

    /// The vector functions compute exactly what glam's methods do (with the platform functions
    /// selected, as in the tests).
    #[test]
    fn vector_functions_match_glam() {
        let vectors = [
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(0.3, -0.4, 0.86),
            DVec3::new(-2.0, 1.0, 0.5),
            DVec3::new(1e-3, 2e-3, 1.0),
            DVec3::new(0.0, 0.0, -1.0),
        ];
        for a in vectors {
            for b in vectors {
                assert!(same(angle_between(a, b), a.angle_between(b)), "{a} {b}");
            }
            let axis = a.normalize();
            for angle in [0.0, 0.1, -1.3, 2.9, 7.0] {
                let ours = quat_from_axis_angle(axis, angle);
                let glam = DQuat::from_axis_angle(axis, angle);
                assert_eq!(
                    ours.to_array().map(f64::to_bits),
                    glam.to_array().map(f64::to_bits)
                );
            }
        }
    }
}
