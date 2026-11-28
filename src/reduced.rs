//! Abiotic reduced gases (#202): hydrogen and methane from volcanoes and from the
//! serpentinization of the sea floor, as a global budget of the air, together with the escape
//! ([`crate::escape`]), which is their main sink on a planet without O₂.
//!
//! - **Volcanic H₂ and CO** ([`Sources::volcanic_hydrogen`]): the mantle CO₂ of arcs and plumes
//!   comes with water, and both with their reduced partners set by the mantle's redox state
//!   ([`ReducedParams::mantle_redox`], log units from the FMQ buffer): H₂/H₂O ≈ 0.01 and CO/CO₂
//!   ≈ 0.03 at FMQ and ~1200 °C (Holland, 2002), both ∝ fO₂^−½. CO counts as H₂: in the air
//!   it becomes CO₂ + H₂ (CO + H₂O by OH); its carbon is part of the carbon cycle's degassing.
//!   The sulfur gases stay a fixed O₂ sink ([`ReducedParams::volcanic_sulfur`]); at FMQ both
//!   together take #212's 0.15 mol O₂ per mol of mantle CO₂.
//! - **Serpentinization** ([`Sources::serpentinization`]): where ridges spread slowly, mantle
//!   peridotite is exposed and serpentinized, and the oxidation of its Fe²⁺ by water releases
//!   H₂ (Cannat et al., 2010; Worman et al., 2016). Per kg of new crust by the full spreading
//!   rate (falling from slow to fast ridges), ~0.4 Tmol H₂/yr for Earth's ridges.
//! - **Abiotic methane** ([`Sources::methane`]): a small share of that H₂ reduces CO₂ to CH₄
//!   (Fischer–Tropsch-type; Etiope & Sherwood Lollar, 2013: 0.001–0.01 Tmol/yr on Earth).
//! - **Sinks**: O₂ where there is some (the escape's O₂ takes them with its other sinks, a
//!   living planet's background O₂ by their oxic lifetimes); without O₂, CH₄ is photolysed and
//!   oxidised by OH from water (`CH₄ + 2 H₂O → CO₂ + 4 H₂`, lifetime ~10⁴ yr) and the hydrogen of
//!   both escapes at its diffusion limit (`f_T = f_H₂O + f_H₂ + 2 f_CH₄`; Hunten, 1973).
//! - **Forcing** ([`forcing`]): CH₄ by Myhre et al. (1998) relative to the pre-industrial 700
//!   ppbv of the climate model's zero forcing, H₂–N₂ collision-induced absorption (Wordsworth &
//!   Pierrehumbert, 2013).
//! - **Budgets**: carbon, hydrogen and the redox (H₂-equivalents) close with the carbon cycle's
//!   exogenic carbon, the ocean's water and the escape's O₂ ([`GasBudget`]).
//!
//! The gases live ~10⁴ yr, far less than a step, so they are close to their steady state in
//! every step (an implicit step). See `notes/issue-202/plan.md`.

use crate::math;

/// Avogadro's number per mol.
const AVOGADRO: f64 = 6.022_140_76e23;
/// Largest forcing (W/m²) of H₂–N₂ collision-induced absorption: it saturates once the
/// window it fills is opaque.
const MAX_CIA_FORCING: f64 = 10.0;
/// Seconds per year.
const YEAR: f64 = 365.25 * 86400.0;
/// Today's O₂ in bar: the reference of the oxic lifetimes.
const PRESENT_OXYGEN: f64 = 0.21;
/// Pre-industrial CH₄ (mixing ratio), the reference of the forcing: the climate model's zero
/// forcing is today's pre-industrial air.
pub const REFERENCE_METHANE: f64 = 700e-9;
/// Forcing of CH₄ in W/m² per √ppbv (Myhre et al., 1998).
const METHANE_FORCING: f64 = 0.036;
/// CH₄ (mixing ratio) above which the forcing is held: the shortwave absorption of CH₄ makes
/// the net forcing grow far more slowly above ~100 ppmv (Byrne & Goldblatt, 2014).
const MAX_METHANE: f64 = 100e-6;
/// Number of the spreading-rate bins (1 cm/yr each) of the new crust.
pub use crate::tectonics::SPREADING_RATE_BINS;

/// Parameters of the reduced gases. The defaults describe today's Earth (a living planet's O₂
/// and CH₄ in the background); [`crate::presets::Biosphere::Abiotic`] removes those.
#[derive(Clone, Debug)]
pub struct ReducedParams {
    /// Whether the reduced gases are simulated (`--no-reduced-gases` turns them off; they also
    /// need the escape).
    pub enabled: bool,
    /// Redox state of the mantle in log units of fO₂ relative to the FMQ buffer: 0 for Earth,
    /// negative for a more reduced planet (−4 is near the iron–wüstite buffer).
    pub mantle_redox: f64,
    /// H₂/H₂O and CO/CO₂ of mantle gases at FMQ (~1200 °C).
    pub hydrogen_ratio: f64,
    pub monoxide_ratio: f64,
    /// Water of the mantle gases of arcs and plumes, mol H₂O per mol of their mantle CO₂.
    pub mantle_water: f64,
    /// O₂ taken by the volcanic sulfur gases (H₂S, SO₂), mol O₂ per mol of mantle CO₂: a fixed
    /// sink of the escape's O₂, the rest of #212's volcanic sink besides the H₂ and CO.
    pub volcanic_sulfur: f64,
    /// Serpentinized peridotite per kg of new oceanic crust at slow ridges (full spreading rate
    /// at most [`Self::slow_rate`]); it falls linearly to 0 at [`Self::fast_rate`] (cm/yr).
    pub serpentinite: f64,
    pub slow_rate: f64,
    pub fast_rate: f64,
    /// H₂ released per kg of serpentinized peridotite (mol/kg).
    pub serpentinite_hydrogen: f64,
    /// CH₄ made per mol of serpentinization H₂ (Fischer–Tropsch-type, 4 H₂ per CH₄).
    pub methane_share: f64,
    /// Lifetime (yr) of CH₄ against photolysis and OH in an atmosphere without O₂.
    pub methane_anoxic_lifetime: f64,
    /// Lifetimes (yr) of CH₄ and H₂ against oxidation at today's O₂ (∝ 1/O₂).
    pub methane_oxic_lifetime: f64,
    pub hydrogen_oxic_lifetime: f64,
    /// O₂ (bar) and CH₄ (mixing ratio) of a living planet's biosphere, which this budget does
    /// not simulate (#201, #203): today's 0.21 bar and 700 ppbv; 0 for a barren planet.
    pub background_oxygen: f64,
    pub background_methane: f64,
    /// Forcing of H₂–N₂ collision-induced absorption in W/m² per bar of H₂ and bar of N₂
    /// (∝ p_H₂ p_N₂, at most 10 W/m²).
    pub cia_coefficient: f64,
}

impl Default for ReducedParams {
    fn default() -> Self {
        Self {
            enabled: true,
            mantle_redox: 0.0,
            hydrogen_ratio: 0.01,
            monoxide_ratio: 0.03,
            mantle_water: 10.0,
            volcanic_sulfur: 0.085,
            serpentinite: 0.08,
            slow_rate: 2.0,
            fast_rate: 8.0,
            serpentinite_hydrogen: 0.25,
            methane_share: 0.01,
            methane_anoxic_lifetime: 1e4,
            methane_oxic_lifetime: 10.0,
            hydrogen_oxic_lifetime: 2.0,
            background_oxygen: PRESENT_OXYGEN,
            background_methane: REFERENCE_METHANE,
            cia_coefficient: 40.0,
        }
    }
}

impl ReducedParams {
    /// Sets a parameter by its field name (`enabled` as 0 or 1).
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if name == "enabled" {
            self.enabled = value != 0.0;
            return Ok(());
        }
        let field = match name {
            "mantle_redox" => &mut self.mantle_redox,
            "hydrogen_ratio" => &mut self.hydrogen_ratio,
            "monoxide_ratio" => &mut self.monoxide_ratio,
            "mantle_water" => &mut self.mantle_water,
            "volcanic_sulfur" => &mut self.volcanic_sulfur,
            "serpentinite" => &mut self.serpentinite,
            "slow_rate" => &mut self.slow_rate,
            "fast_rate" => &mut self.fast_rate,
            "serpentinite_hydrogen" => &mut self.serpentinite_hydrogen,
            "methane_share" => &mut self.methane_share,
            "methane_anoxic_lifetime" => &mut self.methane_anoxic_lifetime,
            "methane_oxic_lifetime" => &mut self.methane_oxic_lifetime,
            "hydrogen_oxic_lifetime" => &mut self.hydrogen_oxic_lifetime,
            "background_oxygen" => &mut self.background_oxygen,
            "background_methane" => &mut self.background_methane,
            "cia_coefficient" => &mut self.cia_coefficient,
            _ => anyhow::bail!("unknown reduced-gas parameter {name}"),
        };
        *field = value;
        Ok(())
    }

    /// Serpentinized peridotite per kg of new crust at the full spreading rate `rate` (cm/yr).
    pub fn serpentinite_at(&self, rate: f64) -> f64 {
        let span = self.fast_rate - self.slow_rate;
        let share = if span > 0.0 {
            ((self.fast_rate - rate) / span).clamp(0.0, 1.0)
        } else {
            f64::from(u8::from(rate <= self.slow_rate))
        };
        self.serpentinite.max(0.0) * share
    }

    /// H₂ and CO per mol of mantle CO₂ (mol/mol): `w x_H/(1 + x_H)` and `x_C/(1 + x_C)` with
    /// the ratios at FMQ scaled by `10^(−ΔFMQ/2)`.
    pub fn volcanic_ratios(&self) -> (f64, f64) {
        let scale = math::powf(10.0, -0.5 * self.mantle_redox);
        let x_h = self.hydrogen_ratio.max(0.0) * scale;
        let x_c = self.monoxide_ratio.max(0.0) * scale;
        (
            self.mantle_water.max(0.0) * x_h / (1.0 + x_h),
            x_c / (1.0 + x_c),
        )
    }
}

/// The mass-weighted serpentinite per kg of the new crust of a step, from the new crust per
/// spreading-rate bin (kg; bin `i` holds `i` to `i + 1` cm/yr). 0 without new crust.
pub fn serpentinite_share(params: &ReducedParams, by_rate: &[f64]) -> f64 {
    let total: f64 = by_rate.iter().map(|m| m.max(0.0)).sum();
    if total <= 0.0 {
        return 0.0;
    }
    by_rate
        .iter()
        .enumerate()
        .map(|(bin, m)| m.max(0.0) * params.serpentinite_at(bin as f64 + 0.5))
        .sum::<f64>()
        / total
}

/// The sources of a step in mol/yr.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sources {
    /// Volcanic H₂ and CO (as H₂: CO + H₂O → CO₂ + H₂) of arcs and plumes.
    pub volcanic_hydrogen: f64,
    pub volcanic_monoxide: f64,
    /// H₂ released by serpentinization (before the share that becomes CH₄).
    pub serpentinization: f64,
    /// CH₄ made from that H₂ (4 H₂ each).
    pub methane: f64,
}

impl Sources {
    /// The sources from the mantle CO₂ of arcs and plumes (mol C/yr), the new oceanic crust
    /// (kg/yr) and its serpentinite share (kg per kg).
    pub fn new(
        params: &ReducedParams,
        mantle_degassing: f64,
        new_crust: f64,
        serpentinite: f64,
    ) -> Self {
        let (hydrogen, monoxide) = params.volcanic_ratios();
        let degassing = mantle_degassing.max(0.0);
        let serpentinization =
            new_crust.max(0.0) * serpentinite.max(0.0) * params.serpentinite_hydrogen.max(0.0);
        // At most all of it (4 H₂ per CH₄).
        let methane =
            (params.methane_share.max(0.0) * serpentinization).min(0.25 * serpentinization);
        Self {
            volcanic_hydrogen: hydrogen * degassing,
            volcanic_monoxide: monoxide * degassing,
            serpentinization,
            methane,
        }
    }

    /// The H₂ that enters the air (mol/yr): volcanic H₂ and CO and what the CH₄ leaves of the
    /// serpentinization's.
    pub fn hydrogen(&self) -> f64 {
        self.volcanic_hydrogen + self.volcanic_monoxide + self.serpentinization - 4.0 * self.methane
    }
}

/// What happened to the gases in a step (mol), or the totals so far.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GasBudget {
    /// From the interior: volcanic H₂, volcanic CO, serpentinization H₂, CH₄ made (its C from
    /// the exogenic carbon).
    pub volcanic_hydrogen: f64,
    pub volcanic_monoxide: f64,
    pub serpentinization: f64,
    pub methane_made: f64,
    /// H₂ and CH₄ oxidised by the escape's O₂ (with its other sinks).
    pub hydrogen_titrated: f64,
    pub methane_titrated: f64,
    /// H₂ and CH₄ oxidised by a living planet's background O₂.
    pub hydrogen_oxidised: f64,
    pub methane_oxidised: f64,
    /// CH₄ photolysed and oxidised without O₂ (→ CO₂ + 4 H₂).
    pub methane_photolysed: f64,
    /// H₂ and CH₄ whose hydrogen escaped (each CH₄ leaves CO₂ + 2 H₂ with two waters).
    pub hydrogen_escaped: f64,
    pub methane_escaped: f64,
    /// Water used by the chemistry, net (mol; negative: made).
    pub water_used: f64,
}

impl GasBudget {
    fn add(&self, other: &GasBudget) -> GasBudget {
        GasBudget {
            volcanic_hydrogen: self.volcanic_hydrogen + other.volcanic_hydrogen,
            volcanic_monoxide: self.volcanic_monoxide + other.volcanic_monoxide,
            serpentinization: self.serpentinization + other.serpentinization,
            methane_made: self.methane_made + other.methane_made,
            hydrogen_titrated: self.hydrogen_titrated + other.hydrogen_titrated,
            methane_titrated: self.methane_titrated + other.methane_titrated,
            hydrogen_oxidised: self.hydrogen_oxidised + other.hydrogen_oxidised,
            methane_oxidised: self.methane_oxidised + other.methane_oxidised,
            methane_photolysed: self.methane_photolysed + other.methane_photolysed,
            hydrogen_escaped: self.hydrogen_escaped + other.hydrogen_escaped,
            methane_escaped: self.methane_escaped + other.methane_escaped,
            water_used: self.water_used + other.water_used,
        }
    }

    /// CH₄ destroyed (mol): its carbon returns to the exogenic carbon as CO₂.
    pub fn methane_destroyed(&self) -> f64 {
        self.methane_titrated
            + self.methane_oxidised
            + self.methane_photolysed
            + self.methane_escaped
    }

    /// Hydrogen atoms (mol) that escaped from H₂ and CH₄.
    pub fn escaped_hydrogen_atoms(&self) -> f64 {
        2.0 * self.hydrogen_escaped + 4.0 * self.methane_escaped
    }
}

/// The state of the gases after a step, for the log and the statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct GasState {
    /// H₂ and CH₄ in the air (mol) and their mixing ratios (abiotic, without the background).
    pub hydrogen: f64,
    pub methane: f64,
    pub hydrogen_ratio: f64,
    pub methane_ratio: f64,
    /// The sources of the step (mol/yr) and the serpentinite share of its new crust.
    pub sources: Sources,
    pub serpentinite: f64,
    /// What happened in the step (mol/yr) and so far (mol).
    pub rates: GasBudget,
    pub totals: GasBudget,
    /// What happened in the step (mol).
    pub step: GasBudget,
    /// Forcing (W/m²) of the CH₄ (with the background, relative to 700 ppbv) and of H₂–N₂ CIA.
    pub methane_forcing: f64,
    pub hydrogen_forcing: f64,
    /// H escaping from the gases (atoms per m² and s).
    pub hydrogen_flux: f64,
    /// CH₄/CO₂ (the haze threshold is ~0.1).
    pub methane_to_co2: f64,
}

impl GasState {
    pub fn forcing(&self) -> f64 {
        self.methane_forcing + self.hydrogen_forcing
    }

    /// One line for the log.
    pub fn summary(&self) -> String {
        let s = &self.sources;
        let r = &self.rates;
        format!(
            "reduced gases: H₂ {:.3e} ({:.3e} mol), CH₄ {:.3e} ({:.3e} mol), CH₄/CO₂ {:.2e} | \
             forcing {:+.3} W/m² (CH₄ {:+.3}, H₂ CIA {:+.4}) | sources Tmol/yr: volcanic H₂ {:.4}, \
             CO {:.4}, serpentinization H₂ {:.4} (serpentinite {:.4} kg/kg), CH₄ {:.5} | sinks \
             Tmol/yr: H₂ escape {:.4}, CH₄ escape {:.5}, CH₄ photolysis {:.5}, by the escape's O₂ \
             H₂ {:.4} CH₄ {:.5}, by background O₂ H₂ {:.4} CH₄ {:.5} | H {:.2e} cm⁻² s⁻¹ | \
             water used {:.3e} mol/yr, so far {:.3e} mol",
            self.hydrogen_ratio,
            self.hydrogen,
            self.methane_ratio,
            self.methane,
            self.methane_to_co2,
            self.forcing(),
            self.methane_forcing,
            self.hydrogen_forcing,
            s.volcanic_hydrogen / 1e12,
            s.volcanic_monoxide / 1e12,
            s.serpentinization / 1e12,
            self.serpentinite,
            s.methane / 1e12,
            r.hydrogen_escaped / 1e12,
            r.methane_escaped / 1e12,
            r.methane_photolysed / 1e12,
            r.hydrogen_titrated / 1e12,
            r.methane_titrated / 1e12,
            r.hydrogen_oxidised / 1e12,
            r.methane_oxidised / 1e12,
            self.hydrogen_flux * 1e-4,
            r.water_used,
            self.totals.water_used,
        )
    }
}

/// The forcing (W/m²) of CH₄ at the mixing ratio `methane` (with the background) relative to
/// the pre-industrial 700 ppbv (Myhre et al., 1998, `0.036 (√M − √M₀)`, M in ppbv; held above
/// 100 ppmv), and of H₂–N₂ collision-induced absorption at `hydrogen` bar of H₂ in `nitrogen`
/// bar of N₂ (`a p_H₂ p_N₂`, held at 10 W/m²).
pub fn forcing(params: &ReducedParams, methane: f64, hydrogen: f64, nitrogen: f64) -> (f64, f64) {
    let ppb = methane.clamp(0.0, MAX_METHANE) * 1e9;
    let methane_forcing = METHANE_FORCING * (ppb.sqrt() - (REFERENCE_METHANE * 1e9).sqrt());
    let hydrogen_forcing =
        (params.cia_coefficient * hydrogen.max(0.0) * nitrogen.max(0.0)).min(MAX_CIA_FORCING);
    (methane_forcing, hydrogen_forcing)
}

/// What a step of the gases needs.
pub struct GasInput {
    pub years: f64,
    /// Planet area (m²) and the column of the non-condensing air besides the gases (mol/m²).
    pub area: f64,
    pub column: f64,
    /// The diffusion limit over the whole planet in mol H₂-equivalents per year at a mixing
    /// ratio of 1: `(b/H_a) A`.
    pub diffusion_limit: f64,
    /// Stratospheric water mixing ratio (part of `f_T`).
    pub water: f64,
    /// What the energy limit leaves for the gases besides the water's escape (mol
    /// H₂-equivalents per year).
    pub energy_room: f64,
    /// Background O₂ and N₂ in bar, and the pressure (bar) of the air without the gases.
    pub background_oxygen: f64,
    pub nitrogen: f64,
    pub pressure: f64,
}

/// The reduced gases (see the module documentation).
#[derive(Clone, Debug)]
pub struct Gases {
    params: ReducedParams,
    /// The new crust per spreading-rate bin so far at the last step (kg).
    previous_by_rate: [f64; SPREADING_RATE_BINS],
    /// The escape per H₂-equivalent of the gases relative to `(b/H_a)` of the last step:
    /// `1/(1 + f_T)`, less where the energy limit held it.
    escape_share: f64,
    state: GasState,
}

impl Gases {
    pub fn new(params: ReducedParams) -> Self {
        Self {
            params,
            previous_by_rate: [0.0; SPREADING_RATE_BINS],
            escape_share: 1.0,
            state: GasState::default(),
        }
    }

    pub fn params(&self) -> &ReducedParams {
        &self.params
    }

    pub fn state(&self) -> &GasState {
        &self.state
    }

    /// Records the new crust per spreading-rate bin at the start, so that the first step sees
    /// only its own.
    pub fn start(&mut self, by_rate: [f64; SPREADING_RATE_BINS]) {
        self.previous_by_rate = by_rate;
    }

    /// The sources of a step from the mantle CO₂ of arcs and plumes (mol/yr), the smoothed new
    /// crust (kg/yr) and the new crust per spreading-rate bin so far (kg). Remembers the
    /// serpentinite share for the state.
    pub fn sources(
        &mut self,
        mantle_degassing: f64,
        new_crust: f64,
        by_rate: [f64; SPREADING_RATE_BINS],
    ) -> (Sources, f64) {
        let mut step = [0.0; SPREADING_RATE_BINS];
        for (bin, mass) in step.iter_mut().enumerate() {
            *mass = (by_rate[bin] - self.previous_by_rate[bin]).max(0.0);
        }
        self.previous_by_rate = by_rate;
        let share = serpentinite_share(&self.params, &step);
        (
            Sources::new(&self.params, mantle_degassing, new_crust, share),
            share,
        )
    }

    /// The pools of the step (mol): the gases in the air and what the sources add.
    pub fn pools(&self, sources: &Sources, years: f64) -> (f64, f64) {
        (
            self.state.hydrogen + years * sources.hydrogen(),
            self.state.methane + years * sources.methane,
        )
    }

    /// The gases after the escape's O₂ took the share `titrated` (0 to 1) of the pools: CH₄
    /// and H₂ by an implicit step. `co2` is the CO₂ partial pressure (bar) for the haze ratio.
    pub fn step(
        &mut self,
        input: &GasInput,
        sources: Sources,
        serpentinite: f64,
        titrated: f64,
        co2: f64,
    ) {
        let params = &self.params;
        let y = input.years;
        let (hydrogen_pool, methane_pool) = self.pools(&sources, y);
        let titrated = titrated.clamp(0.0, 1.0);
        let hydrogen_titrated = titrated * hydrogen_pool;
        let methane_titrated = titrated * methane_pool;
        let hydrogen_free = hydrogen_pool - hydrogen_titrated;
        let methane_free = methane_pool - methane_titrated;

        let oxygen = input.background_oxygen.max(0.0) / PRESENT_OXYGEN;
        let rate = |lifetime: f64| if lifetime > 0.0 { 1.0 / lifetime } else { 0.0 };
        let air = input.area * input.column;
        let limit = input.diffusion_limit.max(0.0);
        let room = input.energy_room.max(0.0);
        // CH₄ escapes per molecule at `(b/H_a) A c/(air)` per year, with `c` the share of the
        // diffusion limit that `1 + f_T` and the energy limit left in the last step (CH₄ is a
        // small part of the escaping hydrogen; linear, so the step stays explicit for it).
        let mut escape =
            limit * self.escape_share / (air + self.state.hydrogen + self.state.methane);
        // CH₄: N = M_f / (1 + y (k_ph + k_ox + k_esc)); what is destroyed is split by the rates.
        let photolysis = rate(params.methane_anoxic_lifetime);
        let methane_oxidation = rate(params.methane_oxic_lifetime) * oxygen;
        // At most what the energy limit leaves: `2 k N(k) ≤ room` with `N(k) = M_f/(1 + y (k_o +
        // k))` gives `k ≤ room (1 + y k_o)/(2 M_f − y room)`.
        let other = photolysis + methane_oxidation;
        if 2.0 * methane_free > y * room {
            escape = escape.min(room * (1.0 + y * other) / (2.0 * methane_free - y * room));
        }
        let methane_sum = photolysis + methane_oxidation + escape;
        let methane = methane_free / (1.0 + y * methane_sum);
        let methane_lost = methane_free - methane;
        let share = |k: f64| {
            if methane_sum > 0.0 {
                methane_lost * k / methane_sum
            } else {
                0.0
            }
        };
        let methane_photolysed = share(photolysis);
        let methane_escaped = share(escape);
        let methane_oxidised = methane_lost - methane_photolysed - methane_escaped;
        // H₂ with what the CH₄ leaves: 4 H₂ per photolysed CH₄, 2 per escaped one. Implicit:
        // `N + y (E(N) + k_ox N) = H_in`, with the escape `E` at the diffusion limit of its
        // mixing ratio, at most what the energy limit leaves (increasing in N: bisection).
        let hydrogen_in = hydrogen_free + 4.0 * methane_photolysed + 2.0 * methane_escaped;
        let hydrogen_oxidation = rate(params.hydrogen_oxic_lifetime) * oxygen;
        let methane_room = (room - 2.0 * methane_escaped / y).max(0.0);
        let escape_of = |n: f64| {
            let total = air + n + methane;
            let f = n / total;
            let f_total = input.water + f + 2.0 * methane / total;
            (limit * f / (1.0 + f_total)).min(methane_room)
        };
        let (mut low, mut high) = (0.0, hydrogen_in);
        for _ in 0..200 {
            let middle = 0.5 * (low + high);
            if middle <= low || middle >= high {
                break;
            }
            if middle + y * (escape_of(middle) + hydrogen_oxidation * middle) > hydrogen_in {
                high = middle;
            } else {
                low = middle;
            }
        }
        let hydrogen = high;
        let hydrogen_lost = hydrogen_in - hydrogen;
        let escaping = escape_of(hydrogen);
        let oxidising = hydrogen_oxidation * hydrogen;
        let hydrogen_escaped = if escaping + oxidising > 0.0 {
            hydrogen_lost * escaping / (escaping + oxidising)
        } else {
            0.0
        };
        let hydrogen_oxidised = hydrogen_lost - hydrogen_escaped;
        // The share of the diffusion limit the gases reached (for the CH₄ of the next step).
        let total = air + hydrogen + methane;
        let f_gases = (hydrogen + 2.0 * methane) / total;
        let diffusion = limit * f_gases / (1.0 + input.water + f_gases);
        let reached = (hydrogen_escaped + 2.0 * methane_escaped) / y;
        self.escape_share = if diffusion > 0.0 {
            (reached / diffusion).min(1.0) / (1.0 + input.water + f_gases)
        } else {
            1.0 / (1.0 + input.water)
        };

        // Water: serpentinization takes one per H₂ (its O goes into the rock), CO one (CO +
        // H₂O → CO₂ + H₂), CH₄ made gives two back, CH₄ photolysed or escaped takes two,
        // oxidation by O₂ gives one per H₂ and two per CH₄.
        let water_used = y
            * (sources.serpentinization + sources.volcanic_monoxide - 2.0 * sources.methane)
            + 2.0 * (methane_photolysed + methane_escaped)
            - (hydrogen_titrated + hydrogen_oxidised)
            - 2.0 * (methane_titrated + methane_oxidised);
        let step = GasBudget {
            volcanic_hydrogen: y * sources.volcanic_hydrogen,
            volcanic_monoxide: y * sources.volcanic_monoxide,
            serpentinization: y * sources.serpentinization,
            methane_made: y * sources.methane,
            hydrogen_titrated,
            methane_titrated,
            hydrogen_oxidised,
            methane_oxidised,
            methane_photolysed,
            hydrogen_escaped,
            methane_escaped,
            water_used,
        };
        let per_year = |x: f64| x / y;
        let rates = GasBudget {
            volcanic_hydrogen: sources.volcanic_hydrogen,
            volcanic_monoxide: sources.volcanic_monoxide,
            serpentinization: sources.serpentinization,
            methane_made: sources.methane,
            hydrogen_titrated: per_year(hydrogen_titrated),
            methane_titrated: per_year(methane_titrated),
            hydrogen_oxidised: per_year(hydrogen_oxidised),
            methane_oxidised: per_year(methane_oxidised),
            methane_photolysed: per_year(methane_photolysed),
            hydrogen_escaped: per_year(hydrogen_escaped),
            methane_escaped: per_year(methane_escaped),
            water_used: per_year(water_used),
        };
        let hydrogen_ratio = hydrogen / total;
        let methane_ratio = methane / total;
        let (methane_forcing, hydrogen_forcing) = forcing(
            params,
            params.background_methane.max(0.0) + methane_ratio,
            // Its partial pressure in the whole column.
            hydrogen_ratio * input.pressure * total / air,
            input.nitrogen,
        );
        let hydrogen_flux = step.escaped_hydrogen_atoms() * AVOGADRO / (input.area * y * YEAR);
        self.state = GasState {
            hydrogen,
            methane,
            hydrogen_ratio,
            methane_ratio,
            sources,
            serpentinite,
            rates,
            totals: self.state.totals.add(&step),
            step,
            methane_forcing,
            hydrogen_forcing,
            hydrogen_flux,
            methane_to_co2: if co2 > 0.0 {
                (params.background_methane.max(0.0) + methane_ratio) / co2
            } else {
                0.0
            },
        };
    }

    /// The forcing (W/m²) without any abiotic gas: that of the background CH₄.
    pub fn initial_forcing(&self) -> f64 {
        let (methane, hydrogen) = forcing(&self.params, self.params.background_methane, 0.0, 1.0);
        methane + hydrogen
    }

    /// The H₂-equivalent mixing ratio of the gases in the air (`f_H₂ + 2 f_CH₄`).
    pub fn hydrogen_equivalent(&self) -> f64 {
        self.state.hydrogen_ratio + 2.0 * self.state.methane_ratio
    }

    /// Carbon (mol) the last step gave back to the exogenic carbon: CH₄ destroyed less CH₄
    /// made.
    pub fn carbon_returned(&self) -> f64 {
        let step = &self.state.step;
        step.methane_destroyed() - step.methane_made
    }

    /// Water (mol) the last step's chemistry used (negative: made).
    pub fn water_used(&self) -> f64 {
        self.state.step.water_used
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At FMQ the volcanic H₂ and CO with the sulfur gases take #212's ~0.15 mol O₂ per mol of
    /// mantle CO₂; a reduced mantle gives far more, saturating.
    #[test]
    fn volcanic_ratios_and_redox() {
        let params = ReducedParams::default();
        let (hydrogen, monoxide) = params.volcanic_ratios();
        let oxygen = 0.5 * (hydrogen + monoxide) + params.volcanic_sulfur;
        assert!((oxygen - 0.15).abs() < 0.005, "{oxygen}");
        let reduced = ReducedParams {
            mantle_redox: -4.0,
            ..ReducedParams::default()
        };
        let (h, c) = reduced.volcanic_ratios();
        assert!((h - 5.0).abs() < 1e-9 && (c - 0.75).abs() < 1e-9, "{h} {c}");
        let very = ReducedParams {
            mantle_redox: -12.0,
            ..ReducedParams::default()
        };
        let (h, c) = very.volcanic_ratios();
        assert!(h < 10.0 && c < 1.0);
    }

    /// Serpentinite falls from slow to fast ridges; Earth's spread of rates on 65 Gt/yr of new
    /// crust gives ~0.4 Tmol H₂/yr.
    #[test]
    fn serpentinization_by_spreading_rate() {
        let params = ReducedParams::default();
        assert_eq!(params.serpentinite_at(1.0), params.serpentinite);
        assert_eq!(params.serpentinite_at(8.5), 0.0);
        assert!((params.serpentinite_at(5.0) - 0.5 * params.serpentinite).abs() < 1e-12);
        let mut bins = [0.0; SPREADING_RATE_BINS];
        for (bin, share) in [(1, 0.05), (3, 0.25), (6, 0.30), (10, 0.40)] {
            bins[bin] = share;
        }
        let share = serpentinite_share(&params, &bins);
        let sources = Sources::new(&params, 0.0, 65e12, share);
        assert!(
            (0.25e12..0.55e12).contains(&sources.serpentinization),
            "{sources:?}"
        );
        assert!(
            (1e-3..1e-2).contains(&(sources.methane / 1e12)),
            "{sources:?}"
        );
        assert_eq!(
            serpentinite_share(&params, &[0.0; SPREADING_RATE_BINS]),
            0.0
        );
    }

    /// CH₄ forcing: 0 at 700 ppbv, −0.95 W/m² without, +0.57 at 1.8 ppmv (Myhre), held above
    /// 100 ppmv; H₂–N₂ CIA ~4 W/m² for 0.1 bar of H₂ in 1 bar of N₂, held at 10.
    #[test]
    fn forcing_values() {
        let params = ReducedParams::default();
        assert!(forcing(&params, REFERENCE_METHANE, 0.0, 1.0).0.abs() < 1e-12);
        assert!((forcing(&params, 0.0, 0.0, 1.0).0 + 0.952).abs() < 0.005);
        assert!((forcing(&params, 1.8e-6, 0.0, 1.0).0 - 0.57).abs() < 0.05);
        assert_eq!(
            forcing(&params, 1e-3, 0.0, 1.0).0,
            forcing(&params, MAX_METHANE, 0.0, 1.0).0
        );
        assert!((forcing(&params, 0.0, 0.1, 1.0).1 - 4.0).abs() < 1e-9);
        assert_eq!(forcing(&params, 0.0, 10.0, 3.0).1, MAX_CIA_FORCING);
    }
}
