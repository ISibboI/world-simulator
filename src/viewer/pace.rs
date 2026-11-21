//! The pace of the simulation: the measured time per step, and a hint that proposes other
//! icosphere subdivisions when steps take too long or are very quick.
//!
//! The simulation sends the time since its previous update with every surface update (see
//! `crate::simulate`). The view takes the median of the last [`WINDOW`] steps, after skipping
//! the first [`SKIPPED`] (on the web the first step is slower: compilation of the hot code and
//! the first upload with the triangles). The target is 0.25–1 s per step (1–4 steps per
//! second). A hint appears if the median is above [`SLOW`] or below [`FAST`]; it proposes the
//! subdivisions predicted to take at most [`AIM`] per step, and never fewer than
//! [`MIN_RECOMMENDED`]. Early in a run it offers to restart with them; later it only informs,
//! since a restart would throw the run away.
//!
//! Step times grow during a run (more tectonic substeps as the plates speed up), so the margins
//! are wide: see `notes/issue-78/research.md` for the measurements behind the numbers.

use std::collections::VecDeque;

use eframe::egui;

use crate::Cli;
use crate::hash::HashMap;

/// Steps per median.
const WINDOW: usize = 10;
/// Steps skipped at the start of a simulation.
const SKIPPED: usize = 2;
/// A hint about slow steps appears above this median step time in seconds…
const SLOW: f64 = 1.2;
/// …and one about quick steps below this one.
const FAST: f64 = 0.2;
/// The proposed subdivisions are predicted to take at most this long per step.
const AIM: f64 = 0.8;
/// A shown hint disappears once the median stayed in this range for [`WINDOW`] steps…
const SETTLED: std::ops::RangeInclusive<f64> = 0.3..=0.9;
/// …and a dismissed one only comes back once the step time is this factor further off.
const DISMISSED_FACTOR: f64 = 1.5;
/// Before a full window, this many steps in a row that are slower than [`VERY_SLOW`] seconds
/// give a verdict, so that a slow device does not wait half a minute for the hint.
const QUICK_VERDICT_STEPS: usize = 3;
const VERY_SLOW: f64 = 2.5;
/// Fewest subdivisions ever proposed: fewer are no faster than this by much (the climate runs
/// on its own coarse grid) and look too coarse.
const MIN_RECOMMENDED: u32 = 5;
/// Most subdivisions proposed unless the view says otherwise (the web says 6: 7 would need ×4 the
/// memory and upload of 6 there).
pub const MAX_RECOMMENDED: u32 = 7;
/// The hint offers a restart only in this first fraction of the simulated duration.
const EARLY_FRACTION: f64 = 0.1;

/// Factor by which a step gets slower from `level` to `level + 1` subdivisions, as assumed
/// when proposing more. Measured early in runs: ×1.4 from 4 to 5, ×2–2.2 from 5 to 6 (×3 late);
/// above, tectonics (×4 cells and ×2 substeps per level) dominates. Rounded up, so that a
/// proposal to go up is cautious.
fn slower_up(level: u32) -> f64 {
    match level {
        0..=3 => 2.0,
        4 => 1.5,
        5 => 3.0,
        6 => 4.5,
        _ => 5.0,
    }
}

/// Factor by which a step gets quicker from `level` to `level - 1` subdivisions, as assumed
/// when proposing fewer: rounded down, so that a proposal to go down is cautious.
fn quicker_down(level: u32) -> f64 {
    match level {
        0..=4 => 1.5,
        5 => 1.3,
        6 => 2.0,
        7 => 3.0,
        _ => 4.0,
    }
}

/// The predicted step time at `to` subdivisions from `seconds` measured at `from`.
fn predict(from: u32, seconds: f64, to: u32) -> f64 {
    let mut seconds = seconds;
    for level in from..to {
        seconds *= slower_up(level);
    }
    for level in (to + 1..=from).rev() {
        seconds /= quicker_down(level);
    }
    seconds
}

/// Whether a hint proposes fewer or more subdivisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Fewer,
    More,
}

/// The subdivisions to propose instead of `current`, whose median step time is `seconds`, or
/// `None` if there are none that are better. `measured` gives the median measured earlier at
/// other subdivisions with otherwise the same options, which wins over the prediction.
fn recommend(
    current: u32,
    seconds: f64,
    direction: Direction,
    max: u32,
    measured: impl Fn(u32) -> Option<f64>,
) -> Option<u32> {
    let expected = |level| measured(level).unwrap_or_else(|| predict(current, seconds, level));
    match direction {
        Direction::Fewer => {
            // Never back to subdivisions that were measured as too quick.
            let usable = |level| measured(level).is_none_or(|time| time >= FAST);
            (MIN_RECOMMENDED..current)
                .rev()
                .find(|&level| usable(level) && expected(level) <= AIM)
                // None is quick enough: the quickest that is still proposed.
                .or_else(|| {
                    (MIN_RECOMMENDED < current && usable(MIN_RECOMMENDED))
                        .then_some(MIN_RECOMMENDED)
                })
        }
        Direction::More => (current.saturating_add(1).max(MIN_RECOMMENDED)..=max)
            .take_while(|&level| expected(level) <= AIM)
            .last(),
    }
}

/// The median of the last steps of one simulation.
#[derive(Debug, Default)]
struct StepTimes {
    /// Steps received so far, including the skipped ones.
    steps: usize,
    window: VecDeque<f64>,
}

impl StepTimes {
    fn push(&mut self, seconds: f64) {
        self.steps += 1;
        if self.steps <= SKIPPED {
            return;
        }
        if self.window.len() == WINDOW {
            self.window.pop_front();
        }
        self.window.push_back(seconds);
    }

    /// The median of the window once it is full; before, the median of the last
    /// [`QUICK_VERDICT_STEPS`] if they are all very slow.
    fn median(&self) -> Option<f64> {
        if self.window.len() == WINDOW {
            return Some(median(self.window.iter().copied()));
        }
        let recent: Vec<f64> = self
            .window
            .iter()
            .rev()
            .take(QUICK_VERDICT_STEPS)
            .copied()
            .collect();
        (recent.len() == QUICK_VERDICT_STEPS && recent.iter().all(|&time| time > VERY_SLOW))
            .then(|| median(recent.into_iter()))
    }
}

fn median(values: impl Iterator<Item = f64>) -> f64 {
    let mut values: Vec<f64> = values.collect();
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

/// A shown hint.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Hint {
    direction: Direction,
    /// The proposed subdivisions.
    subdivisions: u32,
    /// The median step time that the hint reports.
    seconds: f64,
    /// The predicted step time with the proposed subdivisions.
    expected: f64,
}

/// The step times of the running simulation and the hint about them.
#[derive(Debug)]
pub struct Pace {
    /// The latest median step time per options (without the subdivisions) and subdivisions,
    /// of this and earlier simulations of the session.
    measured: HashMap<(String, u32), f64>,
    /// The options of the running simulation without its subdivisions.
    options: String,
    subdivisions: u32,
    times: StepTimes,
    hint: Option<Hint>,
    /// Steps in a row whose median was in [`SETTLED`] while the hint was shown.
    settled: usize,
    /// The thresholds for a new hint; dismissing a hint moves its threshold further out.
    slow: f64,
    fast: f64,
    /// Most subdivisions proposed.
    max: u32,
}

/// What the user did with the hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintAction {
    /// Restart with these subdivisions.
    Restart(u32),
}

impl Pace {
    /// Measures the simulation with options `cli`, proposing at most `max` subdivisions.
    pub fn new(cli: &Cli, max: u32) -> Self {
        let mut pace = Self {
            measured: HashMap::default(),
            options: String::new(),
            subdivisions: 0,
            times: StepTimes::default(),
            hint: None,
            settled: 0,
            slow: SLOW,
            fast: FAST,
            max,
        };
        pace.start(cli);
        pace
    }

    /// Starts measuring a new simulation.
    pub fn start(&mut self, cli: &Cli) {
        // The options that matter for the step time, without the subdivisions.
        let mut options = cli.clone();
        options.subdivisions = 0;
        options.duration = 0.0;
        options.snapshot_interval = 0.0;
        options.output = Default::default();
        options.width = 0;
        options.river_threshold = 0.0;
        options.exaggeration = 0.0;
        self.options = format!("{options:?}");
        self.subdivisions = cli.subdivisions;
        self.times = StepTimes::default();
        self.hint = None;
        self.settled = 0;
        self.slow = SLOW;
        self.fast = FAST;
    }

    /// The median step time in seconds, once known.
    pub fn median(&self) -> Option<f64> {
        self.times.median()
    }

    /// Takes the time of one step in seconds (`None` if it is unknown or not representative,
    /// e.g. the page was hidden meanwhile).
    pub fn record(&mut self, seconds: Option<f64>) {
        let Some(seconds) = seconds.filter(|seconds| seconds.is_finite() && *seconds >= 0.0) else {
            return;
        };
        self.times.push(seconds);
        let Some(median) = self.times.median() else {
            return;
        };
        self.measured
            .insert((self.options.clone(), self.subdivisions), median);
        let options = &self.options;
        let measured = |level| self.measured.get(&(options.clone(), level)).copied();
        let current = self.subdivisions;
        let max = self.max;
        let hint_for = |direction| {
            let subdivisions = recommend(current, median, direction, max, measured)?;
            Some(Hint {
                direction,
                subdivisions,
                seconds: median,
                expected: measured(subdivisions)
                    .unwrap_or_else(|| predict(current, median, subdivisions)),
            })
        };
        match self.hint {
            Some(hint) => {
                // A hint whose step time is now on the other side of the target says the
                // opposite of the facts: it goes.
                let reversed = match hint.direction {
                    Direction::Fewer => median < *SETTLED.start(),
                    Direction::More => median > *SETTLED.end(),
                };
                if reversed {
                    self.hint = None;
                    self.settled = 0;
                } else if SETTLED.contains(&median) {
                    self.settled += 1;
                    if self.settled >= WINDOW {
                        self.hint = None;
                    }
                } else {
                    self.settled = 0;
                    // The proposal follows the step time; it stays if nothing is better now.
                    self.hint = Some(hint_for(hint.direction).unwrap_or(Hint {
                        seconds: median,
                        ..hint
                    }));
                }
            }
            None => {
                self.settled = 0;
                if median > self.slow {
                    self.hint = hint_for(Direction::Fewer);
                } else if median < self.fast {
                    self.hint = hint_for(Direction::More);
                }
            }
        }
    }

    /// Hides the hint; one in the same direction only comes back if the step time gets a
    /// further [`DISMISSED_FACTOR`] off.
    fn dismiss(&mut self) {
        if let Some(hint) = self.hint.take() {
            match hint.direction {
                Direction::Fewer => self.slow = self.slow.max(hint.seconds) * DISMISSED_FACTOR,
                Direction::More => self.fast = self.fast.min(hint.seconds) / DISMISSED_FACTOR,
            }
        }
    }

    /// Whether a hint would be shown for a simulation at `time` of `duration` Myr.
    pub fn has_hint(&self, time: Option<f64>, duration: f64) -> bool {
        self.hint.is_some() && time.is_some_and(|time| time < duration)
    }

    /// Shows the hint, if there is one, for a simulation at `time` of `duration` Myr; nothing
    /// before the first update or once the simulation finished. Returns what the user chose.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        time: Option<f64>,
        duration: f64,
    ) -> Option<HintAction> {
        if !self.has_hint(time, duration) {
            return None;
        }
        let hint = self.hint?;
        let early = time.is_some_and(|time| time <= EARLY_FRACTION * duration);
        let mut action = None;
        // Opaque, so that it is legible over the planet when the panel is hidden.
        let visuals = ui.visuals();
        let fill = visuals
            .window_fill
            .lerp_to_gamma(visuals.warn_fg_color, 0.15);
        egui::Frame::group(ui.style()).fill(fill).show(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.label(hint_text(&hint, self.subdivisions));
                ui.horizontal(|ui| {
                    if early {
                        if ui
                            .button(format!("Restart with {}", hint.subdivisions))
                            .on_hover_text("Restart the simulation with these subdivisions")
                            .clicked()
                        {
                            action = Some(HintAction::Restart(hint.subdivisions));
                        }
                    } else {
                        ui.label("Choose them in the parameters for the next run.");
                    }
                    if ui.button("Dismiss").clicked() {
                        self.dismiss();
                    }
                });
            });
        });
        action
    }
}

/// The text of a hint for a simulation at `current` subdivisions.
fn hint_text(hint: &Hint, current: u32) -> String {
    let quality = match hint.direction {
        Direction::Fewer => "slow",
        Direction::More => "quick",
    };
    format!(
        "Steps take {:.2} s at {current} subdivisions (target 0.25–1 s), too {quality}. \
         With {} subdivisions about {:.2} s.",
        hint.seconds, hint.subdivisions, hint.expected
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn cli(subdivisions: u32) -> Cli {
        Cli::try_parse_from(["world-simulator", &format!("--subdivisions={subdivisions}")]).unwrap()
    }

    /// Feeds `seconds` for `steps` steps.
    fn feed(pace: &mut Pace, seconds: f64, steps: usize) {
        for _ in 0..steps {
            pace.record(Some(seconds));
        }
    }

    #[test]
    fn predictions_use_the_cautious_factor_in_each_direction() {
        assert!((predict(5, 0.4, 6) - 1.2).abs() < 1e-12);
        assert!((predict(6, 1.2, 5) - 0.6).abs() < 1e-12);
        assert!((predict(3, 0.1, 5) - 0.1 * 2.0 * 1.5).abs() < 1e-12);
        assert_eq!(predict(5, 0.4, 5), 0.4);
    }

    #[test]
    fn proposals_aim_at_0_8_s_and_never_below_5() {
        let none = |_| None;
        // 1.3 s at 6: 5 is predicted at 0.65 s.
        assert_eq!(recommend(6, 1.3, Direction::Fewer, 7, none), Some(5));
        // 3 s at 7: 6 would be 1 s, so 5.
        assert_eq!(recommend(7, 3.0, Direction::Fewer, 7, none), Some(5));
        // Even 5 is too slow: still 5, never 4.
        assert_eq!(recommend(7, 30.0, Direction::Fewer, 7, none), Some(5));
        // Already at 5 or below: nothing to propose.
        assert_eq!(recommend(5, 2.0, Direction::Fewer, 7, none), None);
        assert_eq!(recommend(4, 2.0, Direction::Fewer, 7, none), None);
        // Quick at 5: 6 is predicted at 0.45 s, 7 at 2 s.
        assert_eq!(recommend(5, 0.15, Direction::More, 7, none), Some(6));
        // 0.3 s at 5 would make 6 0.9 s: nothing.
        assert_eq!(recommend(5, 0.3, Direction::More, 7, none), None);
        // Quick at 3: straight to 5, never 4.
        assert_eq!(recommend(3, 0.1, Direction::More, 7, none), Some(5));
        // At the most proposed subdivisions: nothing more.
        assert_eq!(recommend(7, 0.01, Direction::More, 7, none), None);
    }

    #[test]
    fn measured_step_times_win_over_predictions() {
        // 6 was measured at 1.5 s: going up from a quick 5 is not proposed.
        let measured = |level| (level == 6).then_some(1.5);
        assert_eq!(recommend(5, 0.1, Direction::More, 7, measured), None);
        // 5 was measured as too quick: going down from 6 is not proposed.
        let measured = |level| (level == 5).then_some(0.1);
        assert_eq!(recommend(6, 1.5, Direction::Fewer, 7, measured), None);
    }

    #[test]
    fn the_first_steps_are_skipped_and_the_median_needs_a_full_window() {
        let mut times = StepTimes::default();
        times.push(100.0);
        times.push(100.0);
        for _ in 0..WINDOW - 1 {
            times.push(0.5);
        }
        assert_eq!(times.median(), None);
        times.push(0.7);
        assert_eq!(times.median(), Some(0.5));
        // Single outliers do not move the median.
        times.push(10.0);
        assert_eq!(times.median(), Some(0.5));
    }

    #[test]
    fn very_slow_steps_give_a_verdict_early() {
        let mut pace = Pace::new(&cli(6), 7);
        feed(&mut pace, 3.0, SKIPPED + QUICK_VERDICT_STEPS - 1);
        assert_eq!(pace.hint, None);
        pace.record(Some(3.0));
        assert_eq!(pace.hint.map(|hint| hint.subdivisions), Some(5));
    }

    #[test]
    fn steps_in_the_target_range_give_no_hint() {
        for seconds in [0.25, 0.5, 0.8, 1.0, 1.2] {
            let mut pace = Pace::new(&cli(6), 7);
            feed(&mut pace, seconds, 50);
            assert_eq!(pace.hint, None, "{seconds} s");
        }
    }

    #[test]
    fn slow_and_quick_steps_give_hints() {
        let mut pace = Pace::new(&cli(6), 7);
        feed(&mut pace, 1.4, SKIPPED + WINDOW);
        let hint = pace.hint.unwrap();
        assert_eq!((hint.direction, hint.subdivisions), (Direction::Fewer, 5));
        assert!((hint.expected - 0.7).abs() < 1e-12);

        let mut pace = Pace::new(&cli(5), 7);
        feed(&mut pace, 0.1, SKIPPED + WINDOW);
        let hint = pace.hint.unwrap();
        assert_eq!((hint.direction, hint.subdivisions), (Direction::More, 6));
    }

    #[test]
    fn the_hint_does_not_flicker_around_the_threshold() {
        let mut pace = Pace::new(&cli(6), 7);
        feed(&mut pace, 1.3, SKIPPED + WINDOW);
        assert!(pace.hint.is_some());
        // Just below the threshold, but not settled: the hint stays.
        feed(&mut pace, 1.1, 30);
        assert!(pace.hint.is_some());
        // Settled in the target range for a while: it goes.
        feed(&mut pace, 0.8, WINDOW);
        assert!(pace.hint.is_some(), "the median needs to move first");
        feed(&mut pace, 0.8, WINDOW);
        assert_eq!(pace.hint, None);
        // Back to just above the threshold: a new hint.
        feed(&mut pace, 1.25, WINDOW);
        assert!(pace.hint.is_some());
    }

    #[test]
    fn a_dismissed_hint_returns_only_if_the_steps_get_much_slower() {
        let mut pace = Pace::new(&cli(6), 7);
        feed(&mut pace, 1.3, SKIPPED + WINDOW);
        pace.dismiss();
        assert_eq!(pace.hint, None);
        feed(&mut pace, 1.6, 30);
        assert_eq!(pace.hint, None);
        feed(&mut pace, 2.0, 30);
        assert!(pace.hint.is_some());
    }

    #[test]
    fn a_restart_resets_the_hint_but_remembers_measurements() {
        let mut pace = Pace::new(&cli(5), 7);
        feed(&mut pace, 0.1, SKIPPED + WINDOW);
        assert_eq!(pace.hint.map(|hint| hint.subdivisions), Some(6));
        // Accepted; 6 turns out too slow, but 5 was measured as too quick: no way back.
        pace.start(&cli(6));
        assert_eq!(pace.hint, None);
        feed(&mut pace, 1.5, SKIPPED + WINDOW);
        assert_eq!(pace.hint, None);
        // Other options (another seed) do not use those measurements.
        let other =
            Cli::try_parse_from(["world-simulator", "--subdivisions=6", "--seed=1"]).unwrap();
        pace.start(&other);
        feed(&mut pace, 1.5, SKIPPED + WINDOW);
        assert_eq!(pace.hint.map(|hint| hint.subdivisions), Some(5));
    }

    #[test]
    fn invalid_or_missing_times_are_ignored() {
        let mut pace = Pace::new(&cli(6), 7);
        pace.record(None);
        pace.record(Some(f64::NAN));
        pace.record(Some(-1.0));
        assert_eq!(pace.times.steps, 0);
    }

    #[test]
    fn hints_are_shown_only_while_the_simulation_runs() {
        let mut pace = Pace::new(&cli(6), 7);
        feed(&mut pace, 1.4, SKIPPED + WINDOW);
        assert!(pace.has_hint(Some(20.0), 500.0));
        assert!(!pace.has_hint(None, 500.0));
        assert!(!pace.has_hint(Some(500.0), 500.0));
    }

    #[test]
    fn the_web_proposes_at_most_6() {
        assert_eq!(recommend(5, 0.05, Direction::More, 6, |_| None), Some(6));
        assert_eq!(recommend(6, 0.01, Direction::More, 6, |_| None), None);
    }

    #[test]
    fn a_hint_goes_when_the_step_time_crosses_to_the_other_side() {
        let mut pace = Pace::new(&cli(6), 7);
        feed(&mut pace, 1.4, SKIPPED + WINDOW);
        assert_eq!(pace.hint.map(|hint| hint.direction), Some(Direction::Fewer));
        // Much quicker all of a sudden (e.g. the tab got the CPU back): no "too slow" at 0.1 s.
        feed(&mut pace, 0.1, WINDOW);
        assert!(
            pace.hint
                .is_none_or(|hint| hint.direction == Direction::More)
        );
    }

    #[test]
    fn view_only_options_keep_the_measurements() {
        let mut pace = Pace::new(&cli(5), 7);
        feed(&mut pace, 0.1, SKIPPED + WINDOW);
        let other_view = Cli::try_parse_from([
            "world-simulator",
            "--subdivisions=6",
            "--exaggeration=10",
            "--duration=100",
        ])
        .unwrap();
        pace.start(&other_view);
        feed(&mut pace, 1.5, SKIPPED + WINDOW);
        // 5 was measured as too quick with the same physics: no way back.
        assert_eq!(pace.hint, None);
    }
}
