//! Pausing and stepping a running simulation of the 3D view, the same on the desktop thread and
//! in the web worker (#116).
//!
//! A run advances part by part ([`Run::advance_part`]: the parts of [`crate::planet::Planet::step`]),
//! and the simulation reads the requests of the view after every part, so a pause takes effect
//! at the next part boundary, inside a step or between steps, rather than only after the whole
//! step. Between the parts the planet is consistent, and nothing but reads happens there, so a
//! run paused in the middle of a step ends exactly as one that was never paused. A state is only
//! sent to the view between steps: in the middle of a step the planet is part-way through it.
//!
//! - **Pause** stops at the next part boundary. The simulation then tells the view that it has
//!   stopped ([`Halt`]), and whether inside a step.
//! - **Step** while paused between steps takes one step; while paused inside a step (or still on
//!   the way to the pause), finishing that step is the step. Either way it then stops between
//!   steps again.
//! - **Resume** goes on from where it stopped.
//!
//! Every request of the view carries a number, which the [`Halt`] echoes (the latest one the
//! simulation had read when it stopped), so that the view can ignore a report that a later
//! request already overtook.

use std::time::Duration;

use crate::{Cli, Run};

/// What the view asked for: whether the simulation is paused, and the steps asked for since.
#[derive(Debug, Default)]
pub struct Pausing {
    paused: bool,
    /// Steps to complete while paused.
    steps: u32,
    /// The run stopped since the last step began: the time of the step says nothing about the
    /// pace.
    after_pause: bool,
    /// The number of the latest request.
    request: u64,
}

/// The simulation stopped for a pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Halt {
    /// The number of the latest request read before stopping.
    pub request: u64,
    /// Stopped inside a step, not between two.
    pub in_step: bool,
}

/// What [`Pausing::tick`] did.
#[derive(Debug, PartialEq)]
pub enum Tick {
    /// Took a part of a step that is not complete yet.
    Part,
    /// Completed a step: its state is to be sent to the view, with the time since the previous
    /// step if it did not span a pause.
    Step(Option<Duration>),
    /// Stopped for a pause, nothing was computed: wait for the next request.
    Halt(Halt),
    /// The run is complete.
    Complete,
}

impl Pausing {
    /// Pauses or resumes (request number `request`).
    pub fn set_paused(&mut self, paused: bool, request: u64) {
        self.paused = paused;
        if !paused {
            self.steps = 0;
        }
        self.request = request;
    }

    /// One step (request number `request`), if paused: the current one if the run is inside a
    /// step, else the next one.
    pub fn step(&mut self, request: u64) {
        if self.paused {
            self.steps += 1;
        }
        self.request = request;
    }

    /// Whether the run must stay where it is: paused with no step asked for, unless it is
    /// complete (between steps after the last one; a run paused then still ends).
    pub fn holds(&self, run: &Run) -> bool {
        self.paused && self.steps == 0 && (run.in_step() || !run.is_complete())
    }

    /// Takes the next part of the run, unless it must stay where it is.
    pub fn tick(&mut self, run: &mut Run, cli: &Cli) -> Tick {
        if self.holds(run) {
            self.after_pause = true;
            return Tick::Halt(Halt {
                request: self.request,
                in_step: run.in_step(),
            });
        }
        match run.advance_part(cli) {
            None => Tick::Complete,
            Some(None) => Tick::Part,
            Some(Some(step_time)) => {
                if self.paused {
                    self.steps = self.steps.saturating_sub(1);
                }
                Tick::Step((!std::mem::take(&mut self.after_pause)).then_some(step_time))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn cli(output: &std::path::Path) -> Cli {
        Cli::try_parse_from([
            "world-simulator",
            "--headless",
            "--seed=4",
            "--subdivisions=3",
            "--plates=6",
            "--duration=4",
            "--snapshot-interval=100",
            &format!("--output={}", output.display()),
        ])
        .expect("valid options")
    }

    /// Runs a simulation through [`Pausing::tick`], applying `script(tick, pausing)` before every
    /// tick (requests of the view), and returns the ticks and the final state hash.
    fn session(script: impl Fn(usize, &mut Pausing)) -> (Vec<Tick>, Vec<f64>, u64) {
        let output = tempfile::tempdir().expect("temporary directory");
        let cli = cli(output.path());
        let mut run = Run::new(crate::build_planet(&cli).expect("planet"), &cli).expect("run");
        let mut pausing = Pausing::default();
        let mut ticks = Vec::new();
        let mut times = Vec::new();
        for tick in 0.. {
            assert!(tick < 1000, "the run does not end");
            script(tick, &mut pausing);
            match pausing.tick(&mut run, &cli) {
                Tick::Complete => break,
                Tick::Step(time) => {
                    // States are only sent between steps, at whole steps.
                    assert!(!run.in_step());
                    let now = run.planet.tectonics().time();
                    assert!((now - now.round()).abs() < 1e-9, "fractional time {now}");
                    times.push(now);
                    ticks.push(Tick::Step(time.map(|_| Duration::ZERO)));
                }
                other => ticks.push(other),
            }
        }
        (ticks, times, run.planet.state_hash())
    }

    fn halt(request: u64, in_step: bool) -> Tick {
        Tick::Halt(Halt { request, in_step })
    }

    #[test]
    fn a_run_paused_inside_steps_ends_as_an_uninterrupted_one() {
        let (plain, plain_times, plain_hash) = session(|_, _| {});
        assert!(!plain.iter().any(|tick| matches!(tick, Tick::Halt(_))));
        assert_eq!(plain_times, [1.0, 2.0, 3.0, 4.0]);
        // A step has several parts.
        let parts = plain.iter().position(|tick| *tick != Tick::Part).unwrap();
        assert!(parts >= 4, "{parts} parts before the first step");

        // Pause inside the first step, finish it with a step, step once more, pause inside the
        // third step again and resume. Halting repeatedly while held changes nothing.
        let (ticks, times, hash) = session(|tick, pausing| match tick {
            2 => pausing.set_paused(true, 1),
            5 | 7 => {}
            8 => pausing.step(2),
            _ if tick == parts + 10 => pausing.step(3),
            _ if tick == parts * 2 + 15 => pausing.set_paused(false, 4),
            _ if tick == parts * 2 + 18 => pausing.set_paused(true, 5),
            _ if tick == parts * 2 + 25 => pausing.set_paused(false, 6),
            _ => {}
        });
        assert_eq!(hash, plain_hash);
        assert_eq!(times, plain_times);
        // Halted inside the first step until the step request, then between steps.
        assert_eq!(
            ticks[2..8],
            [
                halt(1, true),
                halt(1, true),
                halt(1, true),
                halt(1, true),
                halt(1, true),
                halt(1, true)
            ]
        );
        assert!(ticks.contains(&halt(2, false)));
        assert!(ticks.contains(&halt(3, false)));
        assert!(ticks.contains(&halt(5, true)));
        // The steps that spanned a pause have no time.
        let steps: Vec<_> = ticks
            .iter()
            .filter_map(|tick| match tick {
                Tick::Step(time) => Some(time.is_some()),
                _ => None,
            })
            .collect();
        assert_eq!(steps, [false, false, false, true]);
    }

    #[test]
    fn a_run_paused_after_its_last_step_still_ends() {
        // Paused with exactly the steps of the run asked for: it stops after the last one
        // rather than holding the final state.
        let (ticks, times, _) = session(|tick, pausing| {
            if tick == 0 {
                pausing.set_paused(true, 1);
                for request in 2..6 {
                    pausing.step(request);
                }
            }
        });
        assert_eq!(times, [1.0, 2.0, 3.0, 4.0]);
        assert!(!ticks.iter().any(|tick| matches!(tick, Tick::Halt(_))));
    }

    #[test]
    fn steps_only_count_while_paused() {
        let mut pausing = Pausing::default();
        pausing.step(1);
        assert_eq!(pausing.steps, 0);
        pausing.set_paused(true, 2);
        pausing.step(3);
        pausing.step(4);
        assert_eq!(pausing.steps, 2);
        pausing.set_paused(false, 5);
        assert_eq!(pausing.steps, 0);
        assert_eq!(pausing.request, 5);
    }
}
