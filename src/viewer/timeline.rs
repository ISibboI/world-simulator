//! The time bar along the bottom edge of the 3D view: pause and resume the simulation, step one
//! frame back or forward, a slider over the recorded frames of the run (see [`super::history`]),
//! "Live" to follow the newest state again, and the time of the shown frame.
//!
//! Viewing a past frame does not pause the simulation: the slider keeps growing while the view
//! stays on the chosen frame. Stepping forward on the newest frame while the simulation is paused
//! computes one more step. Keys (when no text field has focus): space pauses or resumes, ← and →
//! step, End goes live.
//!
//! On wide screens the bar is one row; on narrow ones (phones) the slider gets a row of its own
//! below the buttons. On touch screens the buttons and the slider are touch-sized (see
//! `UiOptions`).

use eframe::egui;

/// Distance of the bar from the edges of the window, in points.
const MARGIN: f32 = 8.0;
/// Screens narrower than this (in points) get the two-row layout.
const NARROW_SCREEN: f32 = 600.0;

/// What the bar shows.
pub struct TimelineView<'a> {
    /// Positions of the slider: the recorded frames, and the live state if it is newer than
    /// the last of them (when the history stopped recording).
    pub positions: usize,
    /// The position shown or being loaded.
    pub position: usize,
    /// Whether the view follows the newest state.
    pub live: bool,
    /// The simulated time at a position in Myr.
    pub time_of: &'a dyn Fn(usize) -> Option<f64>,
    /// The time of the newest state.
    pub live_time: f64,
    pub paused: bool,
    /// The run is complete: nothing to pause or step.
    pub finished: bool,
    /// A step was requested and has not arrived.
    pub step_pending: bool,
    /// A past frame is being read.
    pub loading: bool,
    /// Why the shown past frame could not be read, if it could not.
    pub error: Option<&'a str>,
    /// A line about the history for the tooltip of the slider.
    pub about: String,
}

/// What the user asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeAction {
    TogglePause,
    Back,
    Forward,
    Seek(usize),
    Live,
}

/// Reads the keys of the time bar, unless a text field has them. They are consumed, so that
/// space does not also press a focused button.
pub fn keys(ctx: &egui::Context) -> Option<TimeAction> {
    if ctx.egui_wants_keyboard_input() {
        return None;
    }
    ctx.input_mut(|input| {
        let none = egui::Modifiers::NONE;
        if input.consume_key(none, egui::Key::Space) {
            Some(TimeAction::TogglePause)
        } else if input.consume_key(none, egui::Key::ArrowLeft) {
            Some(TimeAction::Back)
        } else if input.consume_key(none, egui::Key::ArrowRight) {
            Some(TimeAction::Forward)
        } else if input.consume_key(none, egui::Key::End) {
            Some(TimeAction::Live)
        } else {
            None
        }
    })
}

/// Whether the forward button does anything: the next frame, or a step of the paused
/// simulation on the newest one.
pub fn can_step_forward(view: &TimelineView) -> bool {
    if view.live || view.position + 1 >= view.positions {
        view.paused && !view.finished && !view.step_pending
    } else {
        true
    }
}

/// "123 Myr", or "123 Myr · live 456" for a past frame.
pub fn time_text(view: &TimelineView) -> String {
    let shown = (view.time_of)(view.position).unwrap_or(view.live_time);
    if view.live {
        format!("{shown:.0} Myr")
    } else {
        format!("{shown:.0} Myr · live {:.0}", view.live_time)
    }
}

/// Shows the bar and returns what the user asked for, with the area the bar covers.
pub fn show(ctx: &egui::Context, view: &TimelineView) -> (Option<TimeAction>, egui::Rect) {
    let screen = ctx.content_rect();
    let width = (screen.width() - 2.0 * MARGIN).max(100.0);
    let narrow = screen.width() < NARROW_SCREEN;
    let mut action = None;
    let response = egui::Area::new(egui::Id::unique("time bar"))
        .anchor(egui::Align2::LEFT_BOTTOM, [MARGIN, -MARGIN])
        .show(ctx, |ui| {
            egui::Frame::window(ui.style()).show(ui, |ui| {
                let inner = width - 2.0 * ui.spacing().window_margin.leftf();
                ui.set_width(inner);
                if narrow {
                    ui.horizontal(|ui| {
                        buttons(ui, view, &mut action);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            time_label(ui, view);
                        });
                    });
                    slider(ui, view, &mut action, inner);
                } else {
                    ui.horizontal(|ui| {
                        buttons(ui, view, &mut action);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            time_label(ui, view);
                            let room = ui.available_width();
                            slider(ui, view, &mut action, room);
                        });
                    });
                }
                if let Some(error) = view.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        });
    (action, response.response.rect)
}

fn buttons(ui: &mut egui::Ui, view: &TimelineView, action: &mut Option<TimeAction>) {
    let (label, tip) = if view.paused {
        ("▶", "Resume the simulation (space)")
    } else {
        ("⏸", "Pause the simulation (space)")
    };
    if ui
        .add_enabled(!view.finished, egui::Button::new(label))
        .on_hover_text(tip)
        .on_disabled_hover_text("The simulation is complete")
        .clicked()
    {
        *action = Some(TimeAction::TogglePause);
    }
    if ui
        .add_enabled(view.position > 0, egui::Button::new("⏴"))
        .on_hover_text("One frame back (←)")
        .clicked()
    {
        *action = Some(TimeAction::Back);
    }
    let forward_tip = if view.live || view.position + 1 >= view.positions {
        "Compute one more step (→)"
    } else {
        "One frame forward (→)"
    };
    if ui
        .add_enabled(can_step_forward(view), egui::Button::new("⏵"))
        .on_hover_text(forward_tip)
        .on_disabled_hover_text(if view.step_pending {
            "Computing the step…"
        } else {
            "Pause to step the simulation (→ then computes one step)"
        })
        .clicked()
    {
        *action = Some(TimeAction::Forward);
    }
    if ui
        .add_enabled(!view.live, egui::Button::new("Live"))
        .on_hover_text("Back to the newest state, following the simulation (End)")
        .clicked()
    {
        *action = Some(TimeAction::Live);
    }
}

fn time_label(ui: &mut egui::Ui, view: &TimelineView) {
    let mut text = egui::RichText::new(time_text(view)).monospace();
    if view.loading {
        text = text.weak();
    }
    let label = ui.label(text);
    if view.paused && !view.finished {
        label.on_hover_text("Paused");
    }
}

fn slider(ui: &mut egui::Ui, view: &TimelineView, action: &mut Option<TimeAction>, width: f32) {
    if view.positions < 2 {
        return;
    }
    let mut position = view.position.min(view.positions - 1);
    ui.spacing_mut().slider_width = width.max(40.0);
    let response = ui
        .add(
            egui::Slider::new(&mut position, 0..=view.positions - 1)
                .show_value(false)
                .trailing_fill(true),
        )
        .on_hover_text(&view.about);
    if response.changed() && position != view.position {
        *action = Some(TimeAction::Seek(position));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(time_of: &dyn Fn(usize) -> Option<f64>) -> TimelineView<'_> {
        TimelineView {
            positions: 10,
            position: 9,
            live: true,
            time_of,
            live_time: 9.0,
            paused: false,
            finished: false,
            step_pending: false,
            loading: false,
            error: None,
            about: String::new(),
        }
    }

    #[test]
    fn forward_steps_frames_or_the_paused_simulation() {
        let time_of = |index: usize| Some(index as f64);
        let mut view = view(&time_of);
        // Live and running: nothing to step to.
        assert!(!can_step_forward(&view));
        view.paused = true;
        assert!(can_step_forward(&view));
        view.step_pending = true;
        assert!(!can_step_forward(&view));
        view.step_pending = false;
        view.finished = true;
        assert!(!can_step_forward(&view));
        // A past frame: the next one.
        view.live = false;
        view.position = 3;
        assert!(can_step_forward(&view));
        assert_eq!(time_text(&view), "3 Myr · live 9");
        view.live = true;
        view.position = 9;
        assert_eq!(time_text(&view), "9 Myr");
    }
}
