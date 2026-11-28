//! Regolith (#197): bare rock weathers in place into a loose layer that the rivers carry away,
//! so erosion is weathering-limited where the rock weathers slowly and transport-limited where
//! the regolith is thick.
//!
//! - **Production** (Heimsath et al., 1997): bedrock turns into regolith at `P₀ e^{−H/H*}`, `H`
//!   the bulk thickness of the loose cover over it. The scale `P₀` grows with the temperature
//!   (Arrhenius), the runoff and frost cracking, and is larger on mafic rock
//!   ([`RegolithParams::production_scale`]).
//! - **Transport**: the regolith is carried away at the capacity `T = r x` of the stream-power
//!   law (the landscape erodibility `K`, i.e. that of weathered rock; a bedrock-equivalent
//!   rate, so a thick regolith lowers the rock as fast as the model without regolith) where it
//!   covers the rock, `T (1 − e^{−H/H*})`; bare rock (the fraction `e^{−H/H*}`, the same
//!   exponential as the production, as in the SPACE model of Shobe et al. 2017) is lowered by the
//!   production and by the direct erosion of fresh rock at
//!   [`RegolithParams::fresh_factor`] times `K`.
//! - With `u = e^{H/H*}` the cover follows the linear `H* u' = A + T − T u` (`A` the bulk
//!   regolith made per year), solved exactly over the step ([`evolve`]).
//!
//! The regolith is stored in the sediment layer (it is loose material of the same mass per
//! rock-equivalent metre); a per-cell label marks the bottom part of the cover as regolith.
//! See `notes/issue-197/plan.md`.

use crate::isostasy::{SEDIMENT_GRAIN_DENSITY, SEDIMENT_ROCK_DENSITY};
use crate::math;

/// Molar gas constant in J/(mol K).
const GAS_CONSTANT: f64 = 8.314;
/// 0 °C in K.
const ZERO_CELSIUS: f64 = 273.15;
/// Porosity of regolith (rubble, grus and saprolite-like weathered rock: 0.3–0.5).
const REGOLITH_POROSITY: f64 = 0.4;
/// Mass per bulk volume of regolith (kg/m³).
const REGOLITH_BULK_DENSITY: f64 = SEDIMENT_GRAIN_DENSITY * (1.0 - REGOLITH_POROSITY);
/// Bulk thickness of regolith per metre of rock-equivalent sediment thickness.
pub const BULK_PER_ROCK_EQUIVALENT: f64 = SEDIMENT_ROCK_DENSITY / REGOLITH_BULK_DENSITY;
/// Below this transport capacity times the step over `H*` the cover is treated as untransported
/// (the closed form for `T = 0`), where the general one loses precision.
const MIN_TRANSPORT: f64 = 1e-9;
/// Cover thicker than this many `H*` shields the rock completely (`e^{−H/H*}` is taken as 0).
const MAX_COVER: f64 = 600.0;

/// Parameters of the regolith. Lengths in metres, times in years.
#[derive(Clone, Debug)]
pub struct RegolithParams {
    /// Whether bare rock weathers into regolith; otherwise rivers incise bedrock directly at the
    /// erodibility `K` (the model before #197).
    pub enabled: bool,
    /// Production of regolith on bare rock (m/yr of bedrock) at the reference temperature and
    /// runoff on felsic rock: Earth's soil production is 10–100 m/Myr in temperate climates
    /// (Heimsath et al. 1997: 77 m/Myr; Heimsath et al. 2000: ~50 m/Myr) with a "speed limit"
    /// of ~150 m/Myr (Dixon & von Blanckenburg 2012); abiotic production (frost, thermal stress,
    /// salt, hydrolysis, without burrowing and roots) at the lower end.
    pub production: f64,
    /// Bulk thickness (m) over which the production and the shielding of the rock fall by e
    /// (Heimsath et al. 1997: ~0.5 m).
    pub depth: f64,
    /// Activation energy (J/mol) of the production: chemical weakening of the rock and physical
    /// breakdown together, so weaker than that of dissolution (40–75 kJ/mol); soil production
    /// rises with temperature, but not as steeply (Dixon et al. 2009).
    pub activation_energy: f64,
    /// Reference annual mean temperature (°C) and runoff (m/yr) of [`Self::production`].
    pub reference_temperature: f64,
    pub reference_runoff: f64,
    /// Exponent of the runoff: production needs water passing through the rock.
    pub runoff_exponent: f64,
    /// Frost cracking relative to the production at the reference temperature, most effective
    /// at a mean annual temperature of [`Self::frost_temperature`] (°C) within
    /// [`Self::frost_width`] (K) (Hales & Roering 2007: −3 to −8 °C; Marshall et al. 2015).
    pub frost: f64,
    pub frost_temperature: f64,
    pub frost_width: f64,
    /// Production on basalt relative to felsic rock (interpolated by the mafic share).
    pub mafic_factor: f64,
    /// Erodibility of fresh bare rock relative to the landscape erodibility `K` (weathered rock):
    /// weathering lowers the tensile strength of rock 2–5× (Murphy et al. 2016) and abrasion
    /// scales with its inverse square (Sklar & Dietrich 2001).
    pub fresh_rock_erodibility: f64,
    /// Bedrock lowering capacity (m/yr) at which fresh rock erodes half-way between
    /// [`Self::fresh_rock_erodibility`] and `K`: where rivers cut fast, hillslopes reach their
    /// threshold and landslides bring fresh bedrock down at the rivers' rate (Montgomery &
    /// Brandon 2002; Larsen et al. 2014: soil production keeps pace up to ~2.5 mm/yr).
    pub threshold_erosion: f64,
    /// Production on bare rock without water (m/yr): salt and thermal weathering of deserts.
    pub dry_production: f64,
}

impl Default for RegolithParams {
    fn default() -> Self {
        Self {
            enabled: true,
            production: 5e-5,
            depth: 0.5,
            activation_energy: 30e3,
            reference_temperature: 15.0,
            reference_runoff: 0.3,
            runoff_exponent: 0.5,
            frost: 0.5,
            frost_temperature: -5.0,
            frost_width: 6.0,
            mafic_factor: 2.0,
            fresh_rock_erodibility: 0.25,
            threshold_erosion: 5e-4,
            dry_production: 2e-6,
        }
    }
}

impl RegolithParams {
    /// Sets the parameter `name` (`regolith`, `regolith_<field>` or `fresh_rock_erodibility`);
    /// returns `None` for names that are not the regolith's.
    pub fn set(&mut self, name: &str, value: f64) -> Option<anyhow::Result<()>> {
        let field = match name {
            "regolith" => {
                self.enabled = value != 0.0;
                return Some(Ok(()));
            }
            "fresh_rock_erodibility" => &mut self.fresh_rock_erodibility,
            "regolith_production" => &mut self.production,
            "regolith_depth" => {
                if value <= 0.0 {
                    return Some(Err(anyhow::anyhow!("regolith_depth must be positive")));
                }
                &mut self.depth
            }
            "regolith_activation_energy" => &mut self.activation_energy,
            "regolith_reference_temperature" => &mut self.reference_temperature,
            "regolith_reference_runoff" => &mut self.reference_runoff,
            "regolith_runoff_exponent" => &mut self.runoff_exponent,
            "regolith_frost" => &mut self.frost,
            "regolith_frost_temperature" => &mut self.frost_temperature,
            "regolith_frost_width" => &mut self.frost_width,
            "regolith_mafic_factor" => &mut self.mafic_factor,
            "regolith_threshold_erosion" => &mut self.threshold_erosion,
            "regolith_dry_production" => &mut self.dry_production,
            _ => return None,
        };
        *field = value;
        Some(Ok(()))
    }

    /// The production `P₀` on bare rock (m/yr of bedrock) at the annual mean `temperature` (°C),
    /// `runoff` (m/yr), permanent `ice` cover (0 to 1) and `mafic` share of the rock.
    pub fn production_scale(&self, temperature: f64, runoff: f64, ice: f64, mafic: f64) -> f64 {
        if ice >= 1.0 {
            return 0.0;
        }
        let dry = self.dry_production * (1.0 - ice.max(0.0));
        if runoff <= 0.0 {
            return dry;
        }
        let reference = self.reference_temperature + ZERO_CELSIUS;
        let kelvin = (temperature + ZERO_CELSIUS).max(1.0);
        let arrhenius =
            math::exp(self.activation_energy / GAS_CONSTANT * (1.0 / reference - 1.0 / kelvin));
        let frost = if self.frost > 0.0 && self.frost_width > 0.0 {
            let z = (temperature - self.frost_temperature) / self.frost_width;
            self.frost * math::exp(-z * z)
        } else {
            0.0
        };
        let water = math::powf(runoff / self.reference_runoff, self.runoff_exponent);
        let lithology = 1.0 + mafic.clamp(0.0, 1.0) * (self.mafic_factor - 1.0);
        dry + self.production * lithology * water * (arrhenius + frost) * (1.0 - ice.max(0.0))
    }

    /// Erodibility of fresh rock relative to `K` at the bedrock lowering capacity `capacity`
    /// (m/yr): [`Self::fresh_rock_erodibility`] on slow rivers, rising to 1 where landslides
    /// take fresh bedrock down at the rivers' rate.
    pub fn fresh_factor(&self, capacity: f64) -> f64 {
        let f = self.fresh_rock_erodibility;
        if self.threshold_erosion <= 0.0 {
            return f;
        }
        f + (1.0 - f) * capacity / (capacity + self.threshold_erosion)
    }
}

/// The cover over `tau` years: `u = e^{H/H*}` at the end, and the exposure `∫ e^{−H/H*} dt`
/// (years of bare rock).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evolution {
    pub u_end: f64,
    pub exposure: f64,
}

/// Solves `H* u' = A + T − T u` exactly over `tau` years from `u0` (≥ 1), with the regolith made
/// per year on bare rock `production` (`A`, bulk m/yr), the transport capacity `transport`
/// (`T`, bulk m/yr) and the depth scale `depth` (`H*`, m).
pub fn evolve(u0: f64, production: f64, transport: f64, tau: f64, depth: f64) -> Evolution {
    if tau <= 0.0 {
        return Evolution {
            u_end: u0,
            exposure: 0.0,
        };
    }
    let rate = transport / depth;
    if rate * tau < MIN_TRANSPORT {
        // `u` grows linearly; `∫ dt / (u0 + a t)`.
        let a = production / depth;
        if a <= 0.0 {
            return Evolution {
                u_end: u0,
                exposure: tau / u0,
            };
        }
        let growth = a * tau / u0;
        return Evolution {
            u_end: u0 + a * tau,
            exposure: math::ln_1p(growth) / a,
        };
    }
    let u_eq = 1.0 + production / transport;
    // u(t) = u_eq + (u0 − u_eq) e^{−kt}
    let u_end = u0 - (u_eq - u0) * math::exp_m1(-rate * tau);
    // ∫ dt / u = (τ + ln(u_end / u0) / k) / u_eq
    let exposure = ((tau + math::ln(u_end / u0) / rate) / u_eq).clamp(0.0, tau);
    Evolution { u_end, exposure }
}

/// The bedrock column of a cell, as the regolith step sees it.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    /// Height (m) of the surface above its base level at the start of the regolith phase.
    pub height: f64,
    /// Lowering of the surface per metre of bedrock removed: of the dense lower crust (down to
    /// `dense` metres) and below.
    pub dense: f64,
    pub dense_response: f64,
    pub response: f64,
    /// Density of the bedrock (kg/m³).
    pub density: f64,
}

impl Column {
    /// Height above base after removing `cut` metres of bedrock (≥ 0).
    fn height_after(&self, cut: f64) -> f64 {
        let dense = cut.min(self.dense);
        (self.height - self.dense_response * dense - self.response * (cut - dense)).max(0.0)
    }
}

/// The result of the regolith phase of one cell.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RegolithStep {
    /// Bedrock removed (m): turned into regolith or eroded as fresh rock.
    pub bedrock: f64,
    /// Regolith at the end (rock-equivalent m).
    pub regolith: f64,
    /// Bare-rock exposure (years).
    pub exposure: f64,
    /// Whether the cell ended transport-limited (`A > T`, thicker than `H* ln 2`).
    pub transport_limited: bool,
}

/// The regolith phase of a cell over `tau` years: starting with `regolith` (rock-equivalent m)
/// under `cover_above` (bulk m of deposit that stays, 0 if cut through), with the production
/// scale `p0` (m/yr of bedrock), the stream-power rate `rate` (`K Q^m / L`, 1/yr) and the column.
/// The bedrock removal `c` solves `c = (P₀ + f r x(c)) I(x(c))` with `x` at the end of the step
/// (implicit); a deposit staying on top means no transport of the regolith below it.
pub fn regolith_step(
    params: &RegolithParams,
    column: &Column,
    regolith: f64,
    cover_above: f64,
    p0: f64,
    rate: f64,
    tau: f64,
) -> RegolithStep {
    let depth = params.depth;
    let bulk = regolith * BULK_PER_ROCK_EQUIVALENT;
    let u0 = math::exp(((bulk + cover_above) / depth).min(MAX_COVER));
    // Bulk regolith made per year on bare rock.
    let production = p0 * column.density / REGOLITH_BULK_DENSITY;
    let transported = cover_above <= 0.0;
    // The transport capacity in bulk regolith per year that carries the mass of `r x` metres
    // of bedrock: in the transport-limited limit the bedrock lowers at `r x`, as without
    // regolith.
    let to_bulk = column.density / REGOLITH_BULK_DENSITY;
    let step = |cut: f64| {
        let x = column.height_after(cut);
        let capacity = if transported { rate * x } else { 0.0 };
        let evolution = evolve(u0, production, capacity * to_bulk, tau, depth);
        let lowering = p0 + params.fresh_factor(capacity) * capacity;
        (lowering * evolution.exposure, evolution, p0 > capacity)
    };
    let (high, evolution, _) = step(0.0);
    let (cut, evolution, transport_limited) = if high <= 0.0 {
        (0.0, evolution, true)
    } else {
        let (mut low, mut high) = (0.0, high);
        for _ in 0..60 {
            let mid = 0.5 * (low + high);
            if mid < step(mid).0 {
                low = mid;
            } else {
                high = mid;
            }
        }
        let (_, evolution, limited) = step(low);
        (low, evolution, limited)
    };
    // The regolith at the end, in the cover below any deposit that stays.
    let end_bulk = (depth * math::ln(evolution.u_end) - cover_above).max(0.0);
    let produced = cut * column.density / SEDIMENT_ROCK_DENSITY;
    // Mass: what is carried away is the start plus the production less the end, never negative.
    let end = (end_bulk / BULK_PER_ROCK_EQUIVALENT).min(regolith + produced);
    RegolithStep {
        bedrock: cut,
        regolith: end,
        exposure: evolution.exposure,
        transport_limited,
    }
}

/// Regolith statistics of one step.
#[derive(Clone, Copy, Debug, Default)]
pub struct RegolithStats {
    /// Land cells (not ocean, not lakes) and those that weathered: bare rock lowered by
    /// production or fresh-rock erosion.
    pub land_cells: usize,
    /// Cells where the cover was cut through and the regolith was carried away, by regime at
    /// the end of the step, and the bedrock (m/yr, summed over cells) they lost.
    pub transport_limited_cells: usize,
    pub weathering_limited_cells: usize,
    pub transport_limited_bedrock: f64,
    pub weathering_limited_bedrock: f64,
    /// Bedrock (m/yr, summed) turned into regolith under deposits or on flat land (no
    /// transport).
    pub untransported_bedrock: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Explicit integration of `dH/dt = A e^{−H/H*} − T (1 − e^{−H/H*})` and of the exposure.
    fn integrate(h0: f64, a: f64, t: f64, tau: f64, depth: f64) -> (f64, f64) {
        let steps = 2_000_000;
        let dt = tau / steps as f64;
        let (mut h, mut exposure) = (h0, 0.0);
        for _ in 0..steps {
            let bare = math::exp(-h / depth);
            exposure += bare * dt;
            h += dt * (a * bare - t * (1.0 - bare));
            h = h.max(0.0);
        }
        (h, exposure)
    }

    #[test]
    fn exact_solution_matches_integration() {
        let depth = 0.5;
        for &(h0, a, t, tau) in &[
            (0.0, 1e-4, 1e-5, 1e5),
            (2.0, 1e-4, 1e-3, 1e4),
            (0.3, 5e-5, 0.0, 1e6),
            (1.0, 1e-5, 1e-4, 1e5),
            (0.0, 0.0, 1e-4, 1e4),
        ] {
            let e = evolve(math::exp(h0 / depth), a, t, tau, depth);
            let (h, exposure) = integrate(h0, a, t, tau, depth);
            let h_exact = depth * math::ln(e.u_end);
            assert!((h_exact - h).abs() < 1e-3 * (1.0 + h), "{h_exact} vs {h}");
            assert!(
                (e.exposure - exposure).abs() < 1e-3 * tau,
                "{} vs {exposure}",
                e.exposure
            );
        }
    }

    #[test]
    fn steady_state_thickness() {
        let (a, t, depth) = (2e-4, 5e-5, 0.5);
        let e = evolve(1.0, a, t, 1e7, depth);
        let expected = depth * math::ln(1.0 + a / t);
        assert!((depth * math::ln(e.u_end) - expected).abs() < 1e-9);
        // Bare fraction T / (A + T).
        assert!((e.exposure / 1e7 - t / (a + t)).abs() < 1e-3);
    }

    #[test]
    fn untransported_regolith_grows_logarithmically() {
        let params = RegolithParams::default();
        let column = Column {
            height: 0.0,
            dense: 0.0,
            dense_response: 0.15,
            response: 0.15,
            density: 2800.0,
        };
        let one = regolith_step(&params, &column, 0.0, 0.0, 5e-5, 0.0, 1e6);
        let ten = regolith_step(&params, &column, 0.0, 0.0, 5e-5, 0.0, 1e7);
        assert!(one.regolith > 0.5 && one.regolith < 3.0, "{one:?}");
        assert!(ten.regolith < 1.6 * one.regolith, "{ten:?} vs {one:?}");
        // Nothing is carried away: all the weathered bedrock stays as regolith.
        for step in [one, ten] {
            let produced = step.bedrock * 2800.0 / SEDIMENT_ROCK_DENSITY;
            assert!((produced - step.regolith).abs() < 1e-6 * produced);
        }
    }

    #[test]
    fn regimes() {
        let params = RegolithParams::default();
        let column = Column {
            height: 1000.0,
            dense: 0.0,
            dense_response: 0.15,
            response: 0.15,
            density: 2800.0,
        };
        let tau = 1e6;
        // Fast transport: weathering-limited, bare rock lowered at about P₀ + f r x.
        let rate = 1e-6; // T ≈ 1 mm/yr
        let fast = regolith_step(&params, &column, 0.0, 0.0, 5e-5, rate, tau);
        assert!(!fast.transport_limited);
        let x = column.height_after(fast.bedrock);
        let bare = (5e-5 + params.fresh_factor(rate * x) * rate * x) * tau;
        assert!(
            (fast.bedrock - bare).abs() < 0.15 * bare,
            "{fast:?} vs {bare}"
        );
        assert!(
            fast.bedrock < rate * x * tau,
            "slower than the old bedrock law"
        );
        // Slow transport: transport-limited, the bedrock follows the transport.
        // Over 10 Myr, so that building the regolith at the start is a small part.
        let (rate, tau) = (1e-9, 1e7); // T = 1 m/Myr
        let slow = regolith_step(&params, &column, 0.0, 0.0, 5e-5, rate, tau);
        assert!(slow.transport_limited);
        let x = column.height_after(slow.bedrock);
        assert!(
            (slow.bedrock / tau - rate * x).abs() < 0.3 * rate * x,
            "{slow:?}"
        );
    }

    #[test]
    fn deposits_shield_the_rock() {
        let params = RegolithParams::default();
        let column = Column {
            height: 100.0,
            dense: 0.0,
            dense_response: 0.15,
            response: 0.15,
            density: 2800.0,
        };
        let step = regolith_step(&params, &column, 0.0, 30.0, 5e-5, 1e-7, 1e6);
        assert!(step.bedrock < 1e-12);
    }

    #[test]
    fn production_follows_climate_and_rock() {
        let p = RegolithParams::default();
        let reference = p.production_scale(15.0, 0.3, 0.0, 0.0);
        assert!(((reference - p.dry_production) / p.production - 1.0).abs() < 1e-3);
        assert!(p.production_scale(25.0, 1.0, 0.0, 0.0) > 2.0 * reference);
        assert!(p.production_scale(-20.0, 0.05, 0.0, 0.0) < 0.2 * reference);
        assert!(p.production_scale(30.0, 0.0, 0.0, 0.0) > 0.0);
        // Frost keeps cold wet rock weathering.
        assert!(p.production_scale(-5.0, 0.5, 0.0, 0.0) > 0.4 * reference);
        assert!(p.production_scale(15.0, 0.3, 0.0, 1.0) > 1.9 * reference);
        assert_eq!(p.production_scale(15.0, 0.3, 1.0, 0.0), 0.0);
        assert!(p.production_scale(15.0, 0.0, 0.0, 0.0) < 0.05 * reference);
    }
}
