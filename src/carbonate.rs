//! Where the carbon cycle's carbonate precipitates (#200): the distribution of a step's
//! carbonate over the crust samples of the sea floor, and the capped proportional fills that
//! keep the totals exact.
//!
//! An abiotic ocean has no calcifying plankton, so carbonate forms where the water is
//! supersaturated enough to precipitate it inorganically: warm, shallow shelves and platforms
//! first, deep water (cold, under pressure) can stay undersaturated. The saturation state of the
//! water at the sea floor is `Ω = S e^{γ (T_b − T₀) − z/z_p}`, the precipitation rate per area
//! `k₀ A(T_b) max(Ω − 1, 0)^n`, and the global level `S` is solved so that the precipitation
//! takes the step's carbonate (a "Neritan" ocean, Ridgwell & Zeebe 2005, whose saturation is set
//! by the area that takes the burial flux). A biogenic ocean also exports a share as pelagic ooze
//! above the CCD. See `notes/issue-200/plan.md`.

use crate::math;

/// Molar mass of the carbonate (kg/mol, calcite): one mol of carbon buried is this much rock.
pub const CARBONATE_MOLAR_MASS: f64 = 0.1;
/// Molar mass of CO₂ (kg/mol): the part of the carbonate that came from the exogenic carbon,
/// and that decarbonation releases.
pub const CO2_MOLAR_MASS: f64 = 0.044;
/// Mass fraction of CO₂ in the carbonate.
pub const CO2_FRACTION: f64 = CO2_MOLAR_MASS / CARBONATE_MOLAR_MASS;

/// Molar gas constant in J/(mol K).
const GAS_CONSTANT: f64 = 8.314;
/// 0 °C in K.
const ZERO_CELSIUS: f64 = 273.15;
/// Freezing point of sea water in °C.
const SEA_WATER_FREEZING: f64 = -1.8;
/// Bisection steps of the level solves (fixed, for determinism).
const ITERATIONS: usize = 64;

/// Parameters of the precipitation (part of [`crate::carbon::CarbonParams`]).
#[derive(Clone, Debug)]
pub struct PrecipitationParams {
    /// Rate constant `k₀` (mol/m²/yr) at `Ω − 1 = 1` and the reference temperature.
    pub rate_constant: f64,
    /// Reaction order `n` of the precipitation in `Ω − 1`.
    pub reaction_order: f64,
    /// Activation energy (J/mol) of the precipitation rate.
    pub activation_energy: f64,
    /// Reference temperature (°C) of the rate constant and the saturation level.
    pub reference_temperature: f64,
    /// Relative change of the saturation per kelvin, `γ` (1/K).
    pub saturation_temperature: f64,
    /// Depth (m) over which the pressure lowers the saturation by `e`, `z_p`.
    pub pressure_depth: f64,
    /// Depth (m) over which the bottom water approaches the deep water's temperature.
    pub thermocline_depth: f64,
    /// Elevation (m, negative) up to which a cell takes carbonate: platforms aggrade to the
    /// shallow subtidal and then build outward.
    pub platform_top: f64,
    /// Share of the carbonate exported as pelagic ooze (biogenic ocean; 0 without life).
    pub pelagic_share: f64,
    /// Depth (m) below which pelagic ooze dissolves (the CCD of a biogenic ocean).
    pub ccd: f64,
}

impl Default for PrecipitationParams {
    fn default() -> Self {
        Self {
            rate_constant: 0.2,
            reaction_order: 2.0,
            activation_energy: 40e3,
            reference_temperature: 25.0,
            saturation_temperature: 0.03,
            pressure_depth: 5800.0,
            thermocline_depth: 1000.0,
            platform_top: -5.0,
            pelagic_share: 0.5,
            ccd: 4500.0,
        }
    }
}

/// One sea-floor cell as the precipitation sees it.
#[derive(Clone, Copy, Debug)]
pub struct SeaCell {
    /// Depth of the sea floor below sea level (m, > 0).
    pub depth: f64,
    /// Annual mean surface temperature (°C).
    pub surface_temperature: f64,
}

/// The precipitation of a cell as `a · max(S b − 1, 0)^n` (mol/m²/yr): `a` the rate constant at
/// the bottom temperature, `b` the relative saturation.
#[derive(Clone, Copy, Debug)]
struct Site {
    a: f64,
    b: f64,
}

impl PrecipitationParams {
    fn site(&self, cell: &SeaCell, deep_water_temperature: f64) -> Site {
        let surface = cell.surface_temperature.max(SEA_WATER_FREEZING);
        let deep = deep_water_temperature.max(SEA_WATER_FREEZING);
        let depth = cell.depth.max(0.0);
        let bottom = if self.thermocline_depth > 0.0 {
            deep + (surface - deep) * math::exp(-depth / self.thermocline_depth)
        } else {
            deep
        };
        let reference = self.reference_temperature + ZERO_CELSIUS;
        let a = self.rate_constant
            * math::exp(
                self.activation_energy / GAS_CONSTANT
                    * (1.0 / reference - 1.0 / (bottom + ZERO_CELSIUS)),
            );
        let pressure = if self.pressure_depth > 0.0 {
            depth / self.pressure_depth
        } else {
            0.0
        };
        let b = math::exp(
            self.saturation_temperature * (bottom - self.reference_temperature) - pressure,
        );
        Site { a, b }
    }

    fn rate(&self, site: Site, level: f64) -> f64 {
        let excess = (level * site.b - 1.0).max(0.0);
        let power = if self.reaction_order == 2.0 {
            excess * excess
        } else if self.reaction_order == 1.0 {
            excess
        } else {
            math::powf(excess, self.reaction_order)
        };
        site.a * power
    }
}

/// A unit that takes carbonate: one crust sample of the sea floor and the cells showing it.
#[derive(Clone, Debug)]
pub struct Unit {
    pub cells: Vec<SeaCell>,
    /// Most carbonate (mol) it takes (its accommodation up to the platform top).
    pub room: f64,
}

/// How a step's carbonate was distributed.
#[derive(Clone, Debug, Default)]
pub struct Distribution {
    /// Carbonate (mol) per unit.
    pub amounts: Vec<f64>,
    /// The solved saturation level `S` (Ω at the reference temperature at sea level); NaN if no
    /// carbonate precipitated by saturation.
    pub level: f64,
    /// Carbonate (mol) that found no room and was put above the caps.
    pub overflow: f64,
    /// Saturation Ω at the sea floor of each unit (the mean of its cells; NaN without a level).
    pub omega: Vec<f64>,
}

/// Distributes `total` mol of carbonate over the `units` (each of area `area` m², over
/// `years`): the pelagic share (if any) evenly per area on units shallower than the CCD, the
/// rest by the saturation rule, each unit at most its room. The amounts sum to `total` up to
/// rounding.
pub fn distribute(
    params: &PrecipitationParams,
    units: &[Unit],
    deep_water_temperature: f64,
    area: f64,
    years: f64,
    total: f64,
) -> Distribution {
    let mut amounts = vec![0.0; units.len()];
    if total <= 0.0 || units.is_empty() {
        return Distribution {
            omega: vec![f64::NAN; amounts.len()],
            amounts,
            level: f64::NAN,
            overflow: total.max(0.0),
        };
    }
    let mut room: Vec<f64> = units.iter().map(|unit| unit.room.max(0.0)).collect();
    let mut remaining = total;

    // Pelagic ooze: evenly over the units above the CCD (the mean of their cells).
    let pelagic = total * params.pelagic_share.clamp(0.0, 1.0);
    if pelagic > 0.0 {
        let weights: Vec<f64> = units
            .iter()
            .map(|unit| {
                let above = unit
                    .cells
                    .iter()
                    .filter(|cell| cell.depth < params.ccd)
                    .count();
                above as f64 / unit.cells.len().max(1) as f64
            })
            .collect();
        let given = capped_fill(&weights, &room, pelagic);
        for ((amount, room), given) in amounts.iter_mut().zip(&mut room).zip(given) {
            *amount += given;
            *room -= given;
            remaining -= given;
        }
    }

    // The rest by saturation.
    let sites: Vec<Vec<Site>> = units
        .iter()
        .map(|unit| {
            unit.cells
                .iter()
                .map(|cell| params.site(cell, deep_water_temperature))
                .collect()
        })
        .collect();
    let scale = area * years;
    let take = |level: f64, unit: usize| -> f64 {
        let cells = &sites[unit];
        if cells.is_empty() {
            return 0.0;
        }
        let rate: f64 = cells.iter().map(|&site| params.rate(site, level)).sum();
        rate / cells.len() as f64 * scale
    };
    let total_at = |level: f64| -> f64 {
        (0..units.len())
            .map(|unit| take(level, unit).min(room[unit]))
            .sum()
    };
    let mut level = f64::NAN;
    let mut overflow = 0.0;
    if remaining > 0.0 {
        let max_b = sites
            .iter()
            .flatten()
            .map(|site| site.b)
            .fold(0.0, f64::max);
        if max_b > 0.0 {
            // Bracket: at 1/max_b nothing precipitates; double until the target is reached or
            // the room is full.
            let room_total: f64 = room.iter().sum();
            let target = remaining.min(room_total);
            let mut low = 1.0 / max_b;
            let mut high = 2.0 * low;
            let mut doublings = 0;
            while total_at(high) < target && doublings < 200 {
                low = high;
                high *= 2.0;
                doublings += 1;
            }
            for _ in 0..ITERATIONS {
                let middle = 0.5 * (low + high);
                if total_at(middle) < target {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            level = high;
            let mut given = 0.0;
            let taken: Vec<f64> = (0..units.len())
                .map(|unit| {
                    let amount = take(level, unit).min(room[unit]);
                    given += amount;
                    amount
                })
                .collect();
            // Scale to the exact target (the bisection leaves ~1e-15 relative).
            let factor = if given > 0.0 { target / given } else { 0.0 };
            for (amount, taken) in amounts.iter_mut().zip(&taken) {
                *amount += taken * factor;
            }
            overflow = remaining - target;
            if overflow > 0.0 {
                // No room left: above the caps, in proportion to the rates.
                let rates: Vec<f64> = (0..units.len()).map(|unit| take(level, unit)).collect();
                let sum: f64 = rates.iter().sum();
                for (amount, rate) in amounts.iter_mut().zip(&rates) {
                    *amount += if sum > 0.0 {
                        overflow * rate / sum
                    } else {
                        overflow / units.len() as f64
                    };
                }
            }
        } else {
            // No saturation anywhere (cannot happen with finite temperatures): evenly.
            for amount in &mut amounts {
                *amount += remaining / units.len() as f64;
            }
            overflow = remaining;
        }
    }
    let omega = sites
        .iter()
        .map(|cells| {
            if cells.is_empty() {
                f64::NAN
            } else {
                level * cells.iter().map(|site| site.b).sum::<f64>() / cells.len() as f64
            }
        })
        .collect();
    Distribution {
        amounts,
        level,
        overflow,
        omega,
    }
}

/// Distributes `total` over units in proportion to `weights`, each at most its `cap`: the
/// amounts are `min(w X, cap)` with the level `X` solved, scaled to sum to `total` exactly. If
/// the caps hold less than `total`, every unit gets its cap (the sum is then less than
/// `total`). Units without weight get nothing.
pub fn capped_fill(weights: &[f64], caps: &[f64], total: f64) -> Vec<f64> {
    let mut amounts = vec![0.0; weights.len()];
    if total <= 0.0 {
        return amounts;
    }
    let capacity: f64 = weights
        .iter()
        .zip(caps)
        .filter(|(w, _)| **w > 0.0)
        .map(|(_, c)| c.max(0.0))
        .sum();
    if capacity <= total {
        for ((amount, &w), &c) in amounts.iter_mut().zip(weights).zip(caps) {
            if w > 0.0 {
                *amount = c.max(0.0);
            }
        }
        return amounts;
    }
    let sum_at = |level: f64| -> f64 {
        weights
            .iter()
            .zip(caps)
            .filter(|(w, _)| **w > 0.0)
            .map(|(&w, &c)| (w * level).min(c.max(0.0)))
            .sum()
    };
    let mut low = 0.0;
    let mut high = weights
        .iter()
        .zip(caps)
        .filter(|(w, _)| **w > 0.0)
        .map(|(&w, &c)| c.max(0.0) / w)
        .fold(0.0, f64::max);
    for _ in 0..ITERATIONS {
        let middle = 0.5 * (low + high);
        if sum_at(middle) < total {
            low = middle;
        } else {
            high = middle;
        }
    }
    let mut given = 0.0;
    for ((amount, &w), &c) in amounts.iter_mut().zip(weights).zip(caps) {
        if w > 0.0 {
            *amount = (w * high).min(c.max(0.0));
            given += *amount;
        }
    }
    if given > 0.0 {
        let factor = total / given;
        for amount in &mut amounts {
            *amount *= factor;
        }
    }
    amounts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(depth: f64, temperature: f64, room: f64) -> Unit {
        Unit {
            cells: vec![SeaCell {
                depth,
                surface_temperature: temperature,
            }],
            room,
        }
    }

    /// The amounts sum to the total; warm shallow water takes most, cold deep water nothing.
    #[test]
    fn warm_shallow_water_takes_the_carbonate() {
        let params = PrecipitationParams {
            pelagic_share: 0.0,
            ..PrecipitationParams::default()
        };
        let units = vec![
            unit(20.0, 28.0, 1e30),
            unit(20.0, 5.0, 1e30),
            unit(4000.0, 28.0, 1e30),
            unit(2000.0, 15.0, 1e30),
        ];
        let total = 1e12;
        let d = distribute(&params, &units, 2.0, 1e10, 1e6, total);
        let sum: f64 = d.amounts.iter().sum();
        assert!((sum / total - 1.0).abs() < 1e-12, "{sum}");
        assert!(d.amounts[0] > d.amounts[1], "{:?}", d.amounts);
        assert!(d.amounts[0] > 0.5 * total, "{:?}", d.amounts);
        assert_eq!(d.amounts[2], 0.0, "{:?}", d.amounts);
        assert!(d.level > 0.0 && d.level.is_finite());
        assert!(d.omega[0] > 1.0 && d.omega[2] < 1.0, "{:?}", d.omega);
        assert_eq!(d.overflow, 0.0);
    }

    /// A full cell passes the carbonate on to the next best ones.
    #[test]
    fn room_caps_the_deposit() {
        let params = PrecipitationParams {
            pelagic_share: 0.0,
            ..PrecipitationParams::default()
        };
        let units = vec![unit(20.0, 28.0, 1e10), unit(50.0, 20.0, 1e30)];
        let total = 1e12;
        let d = distribute(&params, &units, 2.0, 1e10, 1e6, total);
        assert!((d.amounts[0] - 1e10).abs() < 1e-3, "{:?}", d.amounts);
        assert!((d.amounts.iter().sum::<f64>() / total - 1.0).abs() < 1e-12);
        // No room at all: the overflow is counted and still placed.
        let units = vec![unit(20.0, 28.0, 0.0)];
        let d = distribute(&params, &units, 2.0, 1e10, 1e6, total);
        assert_eq!(d.overflow, total);
        assert!((d.amounts[0] - total).abs() < 1e-3);
    }

    /// A biogenic ocean puts its pelagic share on the sea floor above the CCD.
    #[test]
    fn pelagic_share_goes_above_the_ccd() {
        let params = PrecipitationParams::default();
        let units = vec![
            unit(3000.0, 2.0, 1e30),
            unit(5500.0, 2.0, 1e30),
            unit(10.0, 28.0, 1e30),
        ];
        let total = 1e12;
        let d = distribute(&params, &units, 2.0, 1e10, 1e6, total);
        assert!(d.amounts[0] >= 0.5 * total * 0.5 - 1.0, "{:?}", d.amounts);
        assert_eq!(d.amounts[1], 0.0);
        assert!((d.amounts.iter().sum::<f64>() / total - 1.0).abs() < 1e-12);
    }

    #[test]
    fn capped_fill_is_exact() {
        let amounts = capped_fill(&[1.0, 2.0, 0.0, 1.0], &[10.0, 1.0, 100.0, 100.0], 15.0);
        assert!((amounts.iter().sum::<f64>() - 15.0).abs() < 1e-12);
        assert!(amounts[1] <= 1.0 + 1e-12);
        assert_eq!(amounts[2], 0.0);
        assert!((amounts[0] - 7.0).abs() < 1e-9 && (amounts[3] - 7.0).abs() < 1e-9);
        // Not enough room: every cap.
        let amounts = capped_fill(&[1.0, 1.0], &[1.0, 2.0], 5.0);
        assert_eq!(amounts, vec![1.0, 2.0]);
    }
}
