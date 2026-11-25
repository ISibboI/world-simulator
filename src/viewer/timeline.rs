//! The time bar along the bottom edge of the 3D view: pause and resume the simulation, step one
//! frame back or forward, a slider over the recorded frames of the run (see [`super::history`]),
//! "Live" to follow the newest state again, and the time of the shown frame.
//!
//! Viewing a past frame does not pause the simulation: the slider keeps growing while the view
//! stays on the chosen frame. Stepping forward on the newest frame while the simulation is paused
//! computes one more step. Keys (when no text field has focus and no drop-down list is open):
//! space pauses or resumes, ← and → step, End goes live. While paused, the desktop simulation
//! writes the snapshot of its paused step only on resume.
//!
//! On wide screens the bar is one row; on narrow ones (phones) the slider gets a row of its own
//! below the buttons, and the time too if it does not fit beside them (see [`rows`]). On touch screens the buttons and the slider are touch-sized (see
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
    /// The run is complete (or failed): nothing to pause or step.
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
    // Not while a text field or a drop-down list (the mode switcher) takes the keys.
    if ctx.text_edit_focused() || ctx.any_popup_open() {
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

/// The narrowest slider worth showing next to the buttons, in points.
const MIN_SLIDER: f32 = 120.0;

/// How the bar is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    /// Buttons, slider and time in one row (wide screens).
    One,
    /// Buttons and time, then the slider below.
    Two,
    /// Buttons, then the time, then the slider: when the time does not fit beside the buttons
    /// (phones with a long text such as "275 Myr · live 317", issue #161).
    Three,
}

/// The layout for a bar with `inner` points of width, buttons and time text of the given widths
/// and `spacing` points between widgets.
fn rows(inner: f32, buttons: f32, text: f32, spacing: f32, narrow: bool) -> Rows {
    let beside = buttons + spacing + text;
    if !narrow && beside + spacing + MIN_SLIDER <= inner {
        Rows::One
    } else if beside <= inner {
        Rows::Two
    } else {
        Rows::Three
    }
}

/// The width of the buttons of [`buttons`], with the spacing between them.
fn buttons_width(ui: &egui::Ui) -> f32 {
    let spacing = ui.spacing();
    let side = spacing.interact_size.y;
    let live = egui::WidgetText::from("Live")
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        )
        .size()
        .x
        + 2.0 * spacing.button_padding.x;
    3.0 * side + live.max(side) + 3.0 * spacing.item_spacing.x
}

/// The width of the time label showing `text`.
fn text_width(ui: &egui::Ui, text: &str) -> f32 {
    egui::WidgetText::from(egui::RichText::new(text).monospace())
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
        .size()
        .x
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
                let text = time_text(view);
                let rows = rows(
                    inner,
                    buttons_width(ui),
                    text_width(ui, &text),
                    ui.spacing().item_spacing.x,
                    narrow,
                );
                match rows {
                    Rows::One => {
                        ui.horizontal(|ui| {
                            buttons(ui, view, &mut action);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    time_label(ui, view, text);
                                    let room = ui.available_width();
                                    slider(ui, view, &mut action, room);
                                },
                            );
                        });
                    }
                    Rows::Two => {
                        ui.horizontal(|ui| {
                            buttons(ui, view, &mut action);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| time_label(ui, view, text),
                            );
                        });
                        slider(ui, view, &mut action, inner);
                    }
                    Rows::Three => {
                        ui.horizontal(|ui| buttons(ui, view, &mut action));
                        time_label(ui, view, text);
                        slider(ui, view, &mut action, inner);
                    }
                }
                if let Some(error) = view.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        });
    (action, response.response.rect)
}

/// The symbols of the buttons, drawn rather than taken from a font (egui's fonts lack them).
#[derive(Debug, Clone, Copy)]
enum Icon {
    Play,
    Pause,
    Back,
    Forward,
}

/// A square button with an icon.
fn icon_button(ui: &mut egui::Ui, icon: Icon, enabled: bool) -> egui::Response {
    let side = ui.spacing().interact_size.y;
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("").min_size(egui::vec2(side, side)),
    );
    let color = ui.style().interact(&response).fg_stroke.color;
    let color = if enabled {
        color
    } else {
        ui.visuals().weak_text_color()
    };
    let rect = egui::Rect::from_center_size(response.rect.center(), egui::Vec2::splat(side * 0.4));
    let painter = ui.painter();
    let triangle = |left: f32, right: f32, pointing_right: bool| {
        let (tip, base) = if pointing_right {
            (right, left)
        } else {
            (left, right)
        };
        egui::Shape::convex_polygon(
            vec![
                egui::pos2(base, rect.top()),
                egui::pos2(tip, rect.center().y),
                egui::pos2(base, rect.bottom()),
            ],
            color,
            egui::Stroke::NONE,
        )
    };
    let bar = |left: f32, width: f32| {
        egui::Shape::rect_filled(
            egui::Rect::from_x_y_ranges(left..=left + width, rect.y_range()),
            0.0,
            color,
        )
    };
    let width = rect.width();
    match icon {
        Icon::Play => {
            painter.add(triangle(rect.left() + 0.1 * width, rect.right(), true));
        }
        Icon::Pause => {
            painter.add(bar(rect.left() + 0.1 * width, 0.3 * width));
            painter.add(bar(rect.left() + 0.6 * width, 0.3 * width));
        }
        Icon::Back => {
            painter.add(bar(rect.left(), 0.2 * width));
            painter.add(triangle(rect.left() + 0.25 * width, rect.right(), false));
        }
        Icon::Forward => {
            painter.add(triangle(rect.left(), rect.right() - 0.25 * width, true));
            painter.add(bar(rect.right() - 0.2 * width, 0.2 * width));
        }
    }
    response
}

fn buttons(ui: &mut egui::Ui, view: &TimelineView, action: &mut Option<TimeAction>) {
    let (icon, tip) = if view.paused {
        (Icon::Play, "Resume the simulation (space)")
    } else {
        (Icon::Pause, "Pause the simulation (space)")
    };
    if icon_button(ui, icon, !view.finished)
        .on_hover_text(tip)
        .on_disabled_hover_text("The simulation is complete")
        .clicked()
    {
        *action = Some(TimeAction::TogglePause);
    }
    if icon_button(ui, Icon::Back, view.position > 0)
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
    if icon_button(ui, Icon::Forward, can_step_forward(view))
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

fn time_label(ui: &mut egui::Ui, view: &TimelineView, text: String) {
    let mut text = egui::RichText::new(text).monospace();
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

    #[test]
    fn time_text_moves_below_the_buttons_when_it_does_not_fit() {
        // Touch buttons (4 × 48 points with spacing) leave a 412-point phone too little room for
        // "275 Myr · live 317", but enough for "275 Myr".
        let inner = 412.0 - 2.0 * MARGIN - 2.0 * 6.0;
        assert_eq!(rows(inner, 216.0, 170.0, 8.0, true), Rows::Three);
        assert_eq!(rows(inner, 216.0, 60.0, 8.0, true), Rows::Two);
        assert_eq!(rows(1200.0, 216.0, 170.0, 8.0, false), Rows::One);
        // A wide window too narrow for a useful slider beside buttons and time.
        assert_eq!(rows(450.0, 216.0, 170.0, 8.0, false), Rows::Two);
    }
}
