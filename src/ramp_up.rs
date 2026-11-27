//! The ramp-up phase of a run (#213): the time the tectonics needs to settle from the generated
//! start into the model's own steady state.
//!
//! The generator starts near that steady state (about its plate count, a heavy-tailed plate
//! size distribution, the steady plume population and mantle memory; see
//! `tectonics::generation` and `tectonics::mantle`), but not in it: boundaries, ages and slabs
//! still sort themselves out over a few tens of Myr, and the degassing rises with them. Until
//! the phase ends, the carbon cycle holds the CO₂ of the start; at its end it calibrates its
//! weathering on the phase's last window ([`crate::carbon::CarbonCycle::end_hold`]), so that it
//! balances the planet's degassing rather than that of the model's transient.
//!
//! The phase ends on a physical criterion: the plate configuration and the degassing have
//! settled. The run is cut into windows of [`RampUpParams::window`]; after each, the means of
//! the window are compared with those of the window before:
//!
//! - the tectonic degassing (ridges, arcs, plumes and metamorphism, without the seafloor
//!   carbon returning at arcs, which depends on the calibration) must not grow by more than
//!   [`RampUpParams::degassing_tolerance`] (one-sided: the transient is a rise; a fall is no
//!   ramp);
//! - the effective number of plates (`1 / Σ aᵢ²` of the area shares, which the tiny plates that
//!   come and go hardly change) must not change by more than
//!   [`RampUpParams::plate_tolerance`] either way (an overshoot is no steady state either).
//!
//! When [`RampUpParams::passes`] windows in a row pass, the phase ends; at
//! [`RampUpParams::max_duration`] it ends anyway (a safety net, with a warning). Everything is
//! computed from the simulation's own state at the end of each step, so the end is as
//! deterministic as the run.

use tracing::{info, warn};

/// Parameters of the ramp-up phase (`--set ramp_up.NAME=VALUE`).
#[derive(Clone, Debug)]
pub struct RampUpParams {
    /// Length of the windows (Myr) whose means are compared; also the span over which the
    /// carbon cycle calibrates at the end.
    pub window: f64,
    /// Largest relative growth of the mean tectonic degassing from one window to the next.
    pub degassing_tolerance: f64,
    /// Largest relative change of the mean effective plate count from one window to the next.
    pub plate_tolerance: f64,
    /// Windows in a row that must pass.
    pub passes: usize,
    /// The phase lasts at least this long (Myr).
    pub min_duration: f64,
    /// The phase ends at this time (Myr) at the latest.
    pub max_duration: f64,
}

impl Default for RampUpParams {
    fn default() -> Self {
        Self {
            window: 10.0,
            degassing_tolerance: 0.05,
            plate_tolerance: 0.1,
            passes: 2,
            min_duration: 20.0,
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

/// Means of one window.
#[derive(Clone, Copy, Debug, Default)]
struct Window {
    years: f64,
    plates: f64,
    degassing: f64,
}

impl Window {
    fn add(&mut self, years: f64, plates: f64, degassing: f64) {
        self.years += years;
        self.plates += years * plates;
        self.degassing += years * degassing;
    }

    /// The time-weighted means (plates, degassing).
    fn means(&self) -> (f64, f64) {
        let years = self.years.max(f64::MIN_POSITIVE);
        (self.plates / years, self.degassing / years)
    }
}

/// The ramp-up phase of a run (see the module documentation).
#[derive(Clone, Debug)]
pub struct RampUp {
    params: RampUpParams,
    status: RampUpStatus,
    /// The window being filled.
    current: Window,
    /// Means of the last complete window.
    previous: Option<(f64, f64)>,
    /// Windows in a row that passed.
    passed: usize,
    /// Whether the start was logged.
    logged: bool,
}

impl RampUp {
    pub fn new(params: RampUpParams) -> Self {
        Self {
            params,
            status: RampUpStatus::Running,
            current: Window::default(),
            previous: None,
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
            "Ramp-up phase started (tectonics settling{}): it ends when {} windows of {} Myr in \
             a row show the tectonic degassing growing by at most {:.0}% and the effective plate \
             count changing by at most {:.0}%, after at least {} Myr, at the latest at {} Myr",
            holds.map(|h| format!("; {h} held")).unwrap_or_default(),
            p.passes,
            p.window,
            100.0 * p.degassing_tolerance,
            100.0 * p.plate_tolerance,
            p.min_duration,
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
        self.current.add(years, plates, degassing);
        if self.current.years < self.params.window * 1e6 - 1.0 {
            if time >= self.params.max_duration - 1e-9 {
                return self.end(time, false);
            }
            return false;
        }
        let (plate_mean, degassing_mean) = self.current.means();
        self.current = Window::default();
        let passed = self.previous.map(|(plates_before, degassing_before)| {
            let growth =
                (degassing_mean - degassing_before) / degassing_mean.max(f64::MIN_POSITIVE);
            let change = (plate_mean - plates_before).abs() / plate_mean.max(f64::MIN_POSITIVE);
            let pass =
                growth <= self.params.degassing_tolerance && change <= self.params.plate_tolerance;
            (pass, growth, change)
        });
        self.previous = Some((plate_mean, degassing_mean));
        let (pass, growth, change) = passed.unwrap_or((false, f64::NAN, f64::NAN));
        self.passed = if pass && time >= self.params.min_duration - 1e-9 {
            self.passed + 1
        } else {
            0
        };
        info!(
            time = %format_args!("{time:.1}"),
            "Ramp-up: window means: effective plates {plate_mean:.1} ({:+.1}%), tectonic \
             degassing {:.2} Tmol/yr ({:+.1}%); {} of {} windows in a row settled",
            100.0 * change,
            degassing_mean / 1e12,
            100.0 * growth,
            self.passed,
            self.params.passes,
        );
        if self.passed >= self.params.passes {
            self.end(time, true)
        } else if time >= self.params.max_duration - 1e-9 {
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
    fn a_settled_signal_ends_after_the_shortest_time() {
        let at = end(|_, _| (12.0, 14e12), 200.0);
        // Windows end at 10, 20, 30: comparisons at 20 and 30.
        assert_eq!(at, Some(30.0));
    }

    /// Step-to-step noise of the size of the model's (±15% degassing, ±2 effective plates):
    /// the means of 10-Myr windows settle.
    #[test]
    fn noise_settles_but_a_ramp_does_not() {
        let noisy = |_: f64, rng: &mut ChaCha8Rng| {
            (
                12.0 + rng.random_range(-2.0..2.0),
                14e12 * (1.0 + rng.random_range(-0.15..0.15)),
            )
        };
        let at = end(noisy, 200.0).expect("noise settles");
        assert!(at <= 60.0, "{at}");
        // The old 12-plate start: the degassing doubles over ~150 Myr.
        let ramp = |t: f64, rng: &mut ChaCha8Rng| {
            (
                6.0 + 6.0 * (t / 150.0).min(1.0),
                (6e12 + 8e12 * (t / 150.0).min(1.0)) * (1.0 + rng.random_range(-0.05..0.05)),
            )
        };
        let at = end(ramp, 200.0).expect("the safety net ends it");
        assert!(at >= 100.0, "{at}");
    }

    #[test]
    fn falling_degassing_is_no_ramp_but_an_overshooting_plate_count_is() {
        let falling = |t: f64, _: &mut ChaCha8Rng| (12.0, 20e12 - 0.05e12 * t);
        assert_eq!(end(falling, 200.0), Some(30.0));
        // A plate count that keeps growing by 20% per window.
        let overshoot = |t: f64, _: &mut ChaCha8Rng| (12.0 * crate::math::exp(t / 55.0), 14e12);
        assert_eq!(end(overshoot, 200.0), Some(150.0));
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
