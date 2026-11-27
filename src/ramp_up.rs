//! The ramp-up phase of a run (#213): the time the tectonics needs to settle from the generated
//! start.
//!
//! The generator starts near the model's steady state (about its plate count, its plate size
//! distribution, the steady plume population and mantle memory; see `tectonics::generation` and
//! `tectonics::mantle`), but not in it: boundaries, ages and slabs still sort themselves out,
//! and the degassing rises with them. Until the phase ends, the carbon cycle holds the CO₂ of
//! the start; at its end it calibrates its weathering on the phase's last
//! [`RampUpParams::span`] ([`crate::carbon::CarbonCycle::end_hold`]), so that it balances the
//! planet's degassing rather than that of the model's transient.
//!
//! The phase ends on a physical criterion: the degassing and the plate configuration have
//! settled. Every [`RampUpParams::window`], the trends over the last [`RampUpParams::span`] are
//! checked (least-squares slopes of the per-step values, times the span, relative to the mean,
//! which averages the step-to-step noise of the grid far better than comparing two short means):
//!
//! - the tectonic degassing (ridges, arcs, plumes and metamorphism, without the seafloor
//!   carbon returning at arcs, which depends on the calibration) must not grow by more than
//!   [`RampUpParams::degassing_tolerance`] (one-sided: the transient is a rise; a fall is no
//!   ramp);
//! - the effective number of plates (`1 / Σ aᵢ²` of the area shares, which the tiny plates that
//!   come and go hardly change) must not change by more than
//!   [`RampUpParams::plate_tolerance`] either way (an overshoot is no steady state either).
//!
//! When [`RampUpParams::passes`] checks in a row pass, the phase ends; at
//! [`RampUpParams::max_duration`] it ends anyway (a safety net, with a warning). Everything is
//! computed from the simulation's own state at the end of each step, so the end is as
//! deterministic as the run. The tolerances were set on the 8-seed runs of #213
//! (`notes/issue-213/results.md`): the generated start of #213 ends after ~30–60 Myr, the old
//! 12-plate start after ~100 Myr.

use std::collections::VecDeque;

use tracing::{info, warn};

/// Parameters of the ramp-up phase (`--set ramp_up.NAME=VALUE`).
#[derive(Clone, Debug)]
pub struct RampUpParams {
    /// Interval (Myr) between the checks.
    pub window: f64,
    /// The span (Myr) whose trends are checked; also the span over which the carbon cycle
    /// calibrates at the end.
    pub span: f64,
    /// Largest relative growth of the tectonic degassing over the span.
    pub degassing_tolerance: f64,
    /// Largest relative change of the effective plate count over the span.
    pub plate_tolerance: f64,
    /// Checks in a row that must pass.
    pub passes: usize,
    /// The phase lasts at least this long (Myr), and at least the span.
    pub min_duration: f64,
    /// The phase ends at this time (Myr) at the latest.
    pub max_duration: f64,
}

impl Default for RampUpParams {
    fn default() -> Self {
        Self {
            window: 10.0,
            span: 30.0,
            degassing_tolerance: 0.05,
            plate_tolerance: 0.25,
            passes: 1,
            min_duration: 30.0,
            max_duration: 150.0,
        }
    }
}

impl RampUpParams {
    /// Sets a parameter by its field name.
    pub fn set(&mut self, name: &str, value: f64) -> anyhow::Result<()> {
        if name == "passes" {
            self.passes = value.max(1.0) as usize;
            return Ok(());
        }
        let field = match name {
            "window" => &mut self.window,
            "span" => &mut self.span,
            "degassing_tolerance" => &mut self.degassing_tolerance,
            "plate_tolerance" => &mut self.plate_tolerance,
            "min_duration" => &mut self.min_duration,
            "max_duration" => &mut self.max_duration,
            _ => anyhow::bail!("unknown ramp-up parameter {name}"),
        };
        *field = value;
        Ok(())
    }
}

/// Whether the ramp-up phase runs or when it ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RampUpStatus {
    /// The tectonics is settling.
    Running,
    /// The phase ended at this time (Myr): `settled` by the criterion, or else at
    /// [`RampUpParams::max_duration`].
    Ended { at: f64, settled: bool },
}

/// A step as the criterion keeps it.
#[derive(Clone, Copy, Debug)]
struct Step {
    /// End of the step (Myr) and its length (years).
    time: f64,
    years: f64,
    plates: f64,
    degassing: f64,
}

/// The least-squares trend of `value` over `steps` (weighted by their lengths): the slope times
/// the span covered, relative to the mean.
fn relative_trend(steps: &VecDeque<Step>, value: impl Fn(&Step) -> f64) -> f64 {
    let weight: f64 = steps.iter().map(|s| s.years).sum();
    if weight <= 0.0 {
        return 0.0;
    }
    let mean_time = steps.iter().map(|s| s.years * s.time).sum::<f64>() / weight;
    let mean = steps.iter().map(|s| s.years * value(s)).sum::<f64>() / weight;
    let (mut covariance, mut variance) = (0.0, 0.0);
    for step in steps {
        let dt = step.time - mean_time;
        covariance += step.years * dt * (value(step) - mean);
        variance += step.years * dt * dt;
    }
    if variance <= 0.0 || mean == 0.0 {
        return 0.0;
    }
    let span = steps.back().map_or(0.0, |s| s.time) - steps.front().map_or(0.0, |s| s.time)
        + steps.front().map_or(0.0, |s| s.years / 1e6);
    covariance / variance * span / mean
}

/// The ramp-up phase of a run (see the module documentation).
#[derive(Clone, Debug)]
pub struct RampUp {
    params: RampUpParams,
    status: RampUpStatus,
    /// The steps of the last span.
    steps: VecDeque<Step>,
    /// Years since the last check.
    since_check: f64,
    /// Checks in a row that passed.
    passed: usize,
    /// Whether the start was logged.
    logged: bool,
}

impl RampUp {
    pub fn new(params: RampUpParams) -> Self {
        Self {
            params,
            status: RampUpStatus::Running,
            steps: VecDeque::new(),
            since_check: 0.0,
            passed: 0,
            logged: false,
        }
    }

    pub fn params(&self) -> &RampUpParams {
        &self.params
    }

    pub fn status(&self) -> RampUpStatus {
        self.status
    }

    pub fn is_running(&self) -> bool {
        self.status == RampUpStatus::Running
    }

    /// Logs the start of the phase once; `holds` names what is held during it, if anything.
    pub fn log_start(&mut self, holds: Option<&str>) {
        if std::mem::replace(&mut self.logged, true) {
            return;
        }
        let p = &self.params;
        info!(
            "Ramp-up phase started (tectonics settling{}): it ends when, over the last {} Myr, \
             the tectonic degassing grew by at most {:.0}% and the effective plate count changed \
             by at most {:.0}% (checked every {} Myr, {} in a row), after at least {} Myr, at the \
             latest at {} Myr",
            holds.map(|h| format!("; {h} held")).unwrap_or_default(),
            p.span,
            100.0 * p.degassing_tolerance,
            100.0 * p.plate_tolerance,
            p.window,
            p.passes,
            p.min_duration.max(p.span),
            p.max_duration,
        );
    }

    /// Records a step of `years` that ended at `time` (Myr) with the effective plate count
    /// `plates` and the tectonic degassing `degassing` (mol/yr). Returns true if the phase
    /// ended with this step.
    pub fn observe(&mut self, time: f64, years: f64, plates: f64, degassing: f64) -> bool {
        if !self.is_running() {
            return false;
        }
        let params = &self.params;
        self.steps.push_back(Step {
            time,
            years,
            plates,
            degassing,
        });
        while self
            .steps
            .front()
            .is_some_and(|oldest| oldest.time <= time - params.span + 1e-9)
        {
            self.steps.pop_front();
        }
        self.since_check += years;
        if self.since_check < params.window * 1e6 - 1.0 {
            if time >= params.max_duration - 1e-9 {
                return self.end(time, false);
            }
            return false;
        }
        self.since_check = 0.0;
        let growth = relative_trend(&self.steps, |s| s.degassing);
        let change = relative_trend(&self.steps, |s| s.plates);
        let years: f64 = self.steps.iter().map(|s| s.years).sum();
        let plate_mean = self.steps.iter().map(|s| s.years * s.plates).sum::<f64>() / years;
        let degassing_mean = self
            .steps
            .iter()
            .map(|s| s.years * s.degassing)
            .sum::<f64>()
            / years;
        let checked = time >= params.min_duration.max(params.span) - 1e-9;
        let pass = checked
            && growth <= params.degassing_tolerance
            && change.abs() <= params.plate_tolerance;
        self.passed = if pass { self.passed + 1 } else { 0 };
        info!(
            time = %format_args!("{time:.1}"),
            "Ramp-up over the last {:.0} Myr: effective plates {plate_mean:.1} ({:+.1}%), \
             tectonic degassing {:.2} Tmol/yr ({:+.1}%); {} of {} checks in a row settled",
            params.span.min(time),
            100.0 * change,
            degassing_mean / 1e12,
            100.0 * growth,
            self.passed,
            params.passes,
        );
        if self.passed >= params.passes {
            self.end(time, true)
        } else if time >= params.max_duration - 1e-9 {
            self.end(time, false)
        } else {
            false
        }
    }

    fn end(&mut self, time: f64, settled: bool) -> bool {
        self.status = RampUpStatus::Ended { at: time, settled };
        if settled {
            info!("Ramp-up phase ended at {time:.1} Myr: the tectonics has settled");
        } else {
            warn!(
                "Ramp-up phase ended at {time:.1} Myr, its longest duration, before the \
                 tectonics settled"
            );
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::ChaCha8Rng;
    use rand::{RngExt, SeedableRng};

    use super::*;

    /// Runs 1-Myr steps of `signal(t) -> (plates, degassing)`; the end time, if any.
    fn end(signal: impl Fn(f64, &mut ChaCha8Rng) -> (f64, f64), until: f64) -> Option<f64> {
        let mut ramp_up = RampUp::new(RampUpParams::default());
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let mut time = 0.0;
        while time < until {
            time += 1.0;
            let (plates, degassing) = signal(time, &mut rng);
            ramp_up.observe(time, 1e6, plates, degassing);
        }
        match ramp_up.status() {
            RampUpStatus::Ended { at, .. } => Some(at),
            RampUpStatus::Running => None,
        }
    }

    #[test]
    fn the_trend_is_relative_to_the_span() {
        let steps: VecDeque<Step> = (1..=30)
            .map(|t| Step {
                time: f64::from(t),
                years: 1e6,
                plates: 10.0,
                degassing: 10.0 + 0.1 * f64::from(t),
            })
            .collect();
        // +3 over 30 Myr around a mean of 11.55.
        let trend = relative_trend(&steps, |s| s.degassing);
        assert!((trend - 3.0 / 11.55).abs() < 1e-9, "{trend}");
        assert_eq!(relative_trend(&steps, |s| s.plates), 0.0);
    }

    #[test]
    fn a_settled_signal_ends_after_the_shortest_time() {
        assert_eq!(end(|_, _| (8.0, 14e12), 200.0), Some(30.0));
    }

    /// Step-to-step noise of the size of the model's (±20% degassing, ±2 effective plates)
    /// settles; a ramp like that of the old 12-plate start (the degassing doubles over
    /// ~150 Myr) does not until it flattens.
    #[test]
    fn noise_settles_but_a_ramp_does_not() {
        let noisy = |_: f64, rng: &mut ChaCha8Rng| {
            (
                8.0 + rng.random_range(-2.0..2.0),
                14e12 * (1.0 + rng.random_range(-0.2..0.2)),
            )
        };
        let at = end(noisy, 200.0).expect("noise settles");
        assert!(at <= 60.0, "{at}");
        let ramp = |t: f64, rng: &mut ChaCha8Rng| {
            (
                8.0,
                (7e12 + 7e12 * (t / 150.0).min(1.0)) * (1.0 + rng.random_range(-0.1..0.1)),
            )
        };
        let at = end(ramp, 200.0).expect("the safety net ends it");
        assert!(at >= 120.0, "{at}");
    }

    #[test]
    fn falling_degassing_is_no_ramp_but_a_growing_plate_count_is() {
        let falling = |t: f64, _: &mut ChaCha8Rng| (8.0, 20e12 - 0.05e12 * t);
        assert_eq!(end(falling, 200.0), Some(30.0));
        // The effective plate count grows by 40% per 30 Myr.
        let growing = |t: f64, _: &mut ChaCha8Rng| (8.0 * crate::math::exp(t / 90.0), 14e12);
        assert_eq!(end(growing, 200.0), Some(150.0));
    }

    #[test]
    fn the_phase_ends_at_its_longest_duration() {
        let mut ramp_up = RampUp::new(RampUpParams {
            max_duration: 15.0,
            ..RampUpParams::default()
        });
        let mut ended = Vec::new();
        for step in 1..=40 {
            let time = f64::from(step);
            if ramp_up.observe(time, 1e6, time, 1e12 * time) {
                ended.push(time);
            }
        }
        assert_eq!(ended, [15.0]);
        assert_eq!(
            ramp_up.status(),
            RampUpStatus::Ended {
                at: 15.0,
                settled: false
            }
        );
    }

    /// A run shorter than the phase never ends it.
    #[test]
    fn a_short_run_stays_in_the_phase() {
        assert_eq!(end(|t, _| (t, 1e12 * t), 25.0), None);
    }
}
