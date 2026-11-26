//! The time bar along the bottom edge of the 3D view: play or pause the view, step one frame back
//! or forward, a slider over the recorded frames of the run (see [`super::history`]), "Live" to
//! follow the newest state again, pause or resume the simulation, and the time of the shown
//! frame.
//!
//! Play and pause act on the view only (issue #120): pausing freezes the view on its frame while
//! the simulation goes on and the slider keeps growing; playing walks forward through the
//! recorded frames until the newest, and then follows the simulation live. Either way the view
//! changes at most [`VIEW_RATE`] times per second (see [`Walk`]) and shows every state in turn:
//! when the simulation is quicker, a live view falls behind and walks on through the recorded
//! frames ("· live" then shows how far the simulation is ahead), live again once it catches up.
//! The user's own moves (slider, ←, →, End, Live) show at once; Live and End jump to the newest
//! state. The simulation has a button of its own (the
//! cog), drawn pressed while the simulation is paused; the time is then shown in the warning
//! colour. Stepping forward on the newest frame while the simulation is paused computes one more
//! step. Keys (when no text field has focus and no drop-down list is open): space plays or pauses
//! the view, P pauses or resumes the simulation, ← and → step, End goes live. While paused, the
//! desktop simulation writes the snapshot of its paused step only on resume.
//!
//! On wide screens the bar is one row; on narrow ones (phones) the slider gets a row of its own
//! below the buttons, and the time too if it does not fit beside them (see [`rows`]). On touch
//! screens the buttons and the slider are touch-sized (see `UiOptions`).
//!
//! The bar collapses (its ⌄ button) into a flat strip with only the expand button, play/pause,
//! the simulation's button and the time; the keys keep working. Like the other panels it starts
//! collapsed on narrow screens, unless the user collapsed or expanded it before: that choice is
//! remembered across restarts and reloads (see [`super::settings`]).

use std::time::Duration;

use eframe::egui;

/// Distance of the bar from the edges of the window, in points.
const MARGIN: f32 = 8.0;
/// Screens narrower than this (in points) get the two-row layout, and the bar starts collapsed.
const NARROW_SCREEN: f32 = 600.0;
/// Margin inside the frame of the collapsed strip, in points: less than a window's, so that the
/// strip is as flat as its buttons allow.
const STRIP_MARGIN: f32 = 3.0;

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
    /// The time of the state shown while the view is live: the newest one the pace of the view
    /// let through (see [`Walk`]).
    pub live_shown: f64,
    /// Frames per second of a playing view.
    pub rate: f64,
    /// Whether the view walks forward or follows the newest state (rather than staying on its
    /// frame).
    pub playing: bool,
    /// Whether the simulation is paused.
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
    /// Play or pause the view.
    TogglePlay,
    /// Pause or resume the simulation.
    ToggleSimulation,
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
            Some(TimeAction::TogglePlay)
        } else if input.consume_key(none, egui::Key::P) {
            Some(TimeAction::ToggleSimulation)
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

/// The default of [`Walk::rate`]: frames per second of a playing view, walking through the
/// recorded frames or following the simulation live.
pub const VIEW_RATE: f64 = 1.0;

/// The pace of a playing view: each frame stays on screen for 1 / [`Walk::rate`] seconds from when
/// it is shown, so slow reads or rebuilds slow a walk through the recorded frames down rather than
/// skipping or flashing frames. A view following the simulation live keeps the same pace (see
/// [`Walk::due_in`]).
#[derive(Debug)]
pub struct Walk {
    playing: bool,
    /// Frames per second; a single value, for a later speed control.
    pub rate: f64,
    /// When the last frame of the view was shown.
    last: Option<web_time::Instant>,
}

impl Default for Walk {
    fn default() -> Self {
        Self {
            playing: false,
            rate: VIEW_RATE,
            last: None,
        }
    }
}

impl Walk {
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Starts walking; the first frame is due at once.
    pub fn play(&mut self) {
        self.playing = true;
        self.last = None;
    }

    /// A live view fell behind the simulation: it walks on at its pace, the next frame one
    /// interval after the shown one.
    pub fn fall_behind(&mut self) {
        self.playing = true;
    }

    /// Stops walking through the recorded frames (the pace of a live view goes on).
    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// How long a frame stays on screen (at least; the rate is taken within 0.001–1000 frames per
    /// second).
    pub fn interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.rate.clamp(1e-3, 1e3))
    }

    /// How long until the view may show its next frame (zero: now).
    pub fn due_in(&self, now: web_time::Instant) -> Duration {
        let interval = self.interval();
        self.last
            .map_or(Duration::ZERO, |last| interval.saturating_sub(now - last))
    }

    /// How long until the next frame is due (zero: now), or `None` if the walk does not go on
    /// by itself: it is stopped, or the frame it asked for is not shown yet (its read wakes the
    /// view).
    pub fn wait(&self, now: web_time::Instant, shown: bool) -> Option<Duration> {
        if !self.playing || !shown {
            return None;
        }
        Some(self.due_in(now))
    }

    /// A frame of the view is on screen from `now` on.
    pub fn shown(&mut self, now: web_time::Instant) {
        self.last = Some(now);
    }
}

/// Whether the forward button does anything: the next frame, or a step of the paused
/// simulation on the newest one.
pub fn can_step_forward(view: &TimelineView) -> bool {
    if view.live {
        view.paused && !view.finished && !view.step_pending
    } else {
        // A past frame, or the newest one with the view paused: forward goes live on it.
        true
    }
}

/// "123 Myr", or "123 Myr · live 456" for a past frame.
pub fn time_text(view: &TimelineView) -> String {
    let shown = (view.time_of)(view.position).unwrap_or(view.live_time);
    if view.live {
        format!("{:.0} Myr", view.live_shown)
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

/// The width of the buttons of [`buttons`] (with the collapse button), with the spacing between them.
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
    5.0 * side + live.max(side) + 5.0 * spacing.item_spacing.x
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

/// Key of the remembered state of the bar (expanded or collapsed) in the settings.
pub const STORAGE_KEY: &str = "world-simulator.time-bar-open";

/// The time bar and whether it is expanded.
#[derive(Default)]
pub struct TimeBar {
    /// Whether the full bar is shown rather than the flat strip: the remembered choice, else
    /// decided by the screen width on the first frame.
    open: Option<bool>,
    /// A choice of the user (the collapse or expand button) not yet remembered.
    chosen: Option<bool>,
}

impl TimeBar {
    /// The bar, expanded or collapsed as the user left it (`stored`), else as fits the screen.
    pub fn new(stored: Option<bool>) -> Self {
        Self {
            open: stored,
            chosen: None,
        }
    }

    /// The state the user last chose with the buttons, if not taken yet: to remember it.
    pub fn take_choice(&mut self) -> Option<bool> {
        self.chosen.take()
    }

    /// Shows the bar (or the collapsed strip) and returns what the user asked for, with the
    /// area the bar covers.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        view: &TimelineView,
    ) -> (Option<TimeAction>, egui::Rect) {
        let screen = ctx.content_rect();
        let open = *self.open.get_or_insert(screen.width() >= NARROW_SCREEN);
        let mut action = None;
        let mut toggle = false;
        let rect = if open {
            show_bar(ctx, view, &mut action, &mut toggle)
        } else {
            show_strip(ctx, view, &mut action, &mut toggle)
        };
        if toggle {
            self.open = Some(!open);
            self.chosen = Some(!open);
            ctx.request_repaint();
        }
        (action, rect)
    }
}

/// The collapsed bar: the expand button, play/pause, the simulation's button and the time in one
/// flat row, as wide as they are.
fn show_strip(
    ctx: &egui::Context,
    view: &TimelineView,
    action: &mut Option<TimeAction>,
    toggle: &mut bool,
) -> egui::Rect {
    egui::Area::new(egui::Id::unique("time strip"))
        .anchor(egui::Align2::LEFT_BOTTOM, [MARGIN, -MARGIN])
        .show(ctx, |ui| {
            egui::Frame::window(ui.style())
                .inner_margin(STRIP_MARGIN)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if icon_button(ui, Icon::Expand, true)
                            .on_hover_text("Show the time bar")
                            .clicked()
                        {
                            *toggle = true;
                        }
                        play_button(ui, view, action);
                        simulation_button(ui, view, action);
                        time_label(ui, view, time_text(view));
                        if view.error.is_some() {
                            ui.colored_label(ui.visuals().error_fg_color, "⚠")
                                .on_hover_text(view.error.unwrap_or_default());
                        }
                        // A little room after the text, as the frame margin is small.
                        ui.add_space(STRIP_MARGIN);
                    });
                });
        })
        .response
        .rect
}

/// The expanded bar across the bottom edge.
fn show_bar(
    ctx: &egui::Context,
    view: &TimelineView,
    action: &mut Option<TimeAction>,
    toggle: &mut bool,
) -> egui::Rect {
    let screen = ctx.content_rect();
    let width = (screen.width() - 2.0 * MARGIN).max(100.0);
    let narrow = screen.width() < NARROW_SCREEN;
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
                            buttons(ui, view, action, toggle);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    time_label(ui, view, text);
                                    let room = ui.available_width();
                                    slider(ui, view, action, room);
                                },
                            );
                        });
                    }
                    Rows::Two => {
                        ui.horizontal(|ui| {
                            buttons(ui, view, action, toggle);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| time_label(ui, view, text),
                            );
                        });
                        slider(ui, view, action, inner);
                    }
                    Rows::Three => {
                        ui.horizontal(|ui| buttons(ui, view, action, toggle));
                        time_label(ui, view, text);
                        slider(ui, view, action, inner);
                    }
                }
                if let Some(error) = view.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        });
    response.response.rect
}

/// The symbols of the buttons, drawn rather than taken from a font (egui's fonts lack them).
#[derive(Debug, Clone, Copy)]
enum Icon {
    /// A cog: the simulation, with pause bars in its hub while it is paused.
    Simulation {
        paused: bool,
    },
    Play,
    Pause,
    Back,
    Forward,
    /// A chevron pointing up: show the bar.
    Expand,
    /// A chevron pointing down: collapse the bar.
    Collapse,
}

/// A square button with an icon.
fn icon_button(ui: &mut egui::Ui, icon: Icon, enabled: bool) -> egui::Response {
    let selected = matches!(icon, Icon::Simulation { paused: true });
    let side = ui.spacing().interact_size.y;
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .min_size(egui::vec2(side, side))
            .selected(selected),
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
        Icon::Simulation { paused } => {
            // Larger than the other icons, so that the teeth and the bars stay legible.
            let center = rect.center();
            let radius = 0.75 * width;
            let ring = 0.55 * radius;
            let stroke = egui::Stroke::new(0.28 * radius, color);
            painter.circle_stroke(center, ring, stroke);
            for tooth in 0..6 {
                let angle = tooth as f32 * std::f32::consts::FRAC_PI_3;
                let direction = egui::vec2(angle.cos(), angle.sin());
                painter.line_segment(
                    [center + direction * ring, center + direction * radius],
                    egui::Stroke::new(0.22 * radius, color),
                );
            }
            if paused {
                let height = 0.5 * ring;
                for x in [-0.22 * ring, 0.22 * ring] {
                    painter.line_segment(
                        [
                            center + egui::vec2(x, -height),
                            center + egui::vec2(x, height),
                        ],
                        egui::Stroke::new(0.25 * ring, color),
                    );
                }
            }
        }
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
        Icon::Expand | Icon::Collapse => {
            let (tip, ends) = if matches!(icon, Icon::Expand) {
                (rect.top() + 0.2 * width, rect.bottom() - 0.2 * width)
            } else {
                (rect.bottom() - 0.2 * width, rect.top() + 0.2 * width)
            };
            painter.add(egui::Shape::line(
                vec![
                    egui::pos2(rect.left(), ends),
                    egui::pos2(rect.center().x, tip),
                    egui::pos2(rect.right(), ends),
                ],
                egui::Stroke::new((0.12 * width).max(1.5), color),
            ));
        }
    }
    response
}

/// Plays or pauses the view.
fn play_button(ui: &mut egui::Ui, view: &TimelineView, action: &mut Option<TimeAction>) {
    let (icon, tip) = if view.playing {
        (
            Icon::Pause,
            "Pause the view on this frame; the simulation goes on (space)".to_owned(),
        )
    } else {
        (
            Icon::Play,
            format!(
                "Play: walk through the frames ({} per second) up to the newest, then follow \
                 the simulation (space)",
                view.rate
            ),
        )
    };
    if icon_button(ui, icon, true).on_hover_text(tip).clicked() {
        *action = Some(TimeAction::TogglePlay);
    }
}

/// Pauses or resumes the simulation.
fn simulation_button(ui: &mut egui::Ui, view: &TimelineView, action: &mut Option<TimeAction>) {
    let tip = if view.paused {
        "The simulation is paused: resume it (P)"
    } else {
        "Pause the simulation (P); the view can still play the recorded frames"
    };
    if icon_button(
        ui,
        Icon::Simulation {
            paused: view.paused && !view.finished,
        },
        !view.finished,
    )
    .on_hover_text(tip)
    .on_disabled_hover_text("The simulation is complete")
    .clicked()
    {
        *action = Some(TimeAction::ToggleSimulation);
    }
}

fn buttons(
    ui: &mut egui::Ui,
    view: &TimelineView,
    action: &mut Option<TimeAction>,
    toggle: &mut bool,
) {
    if icon_button(ui, Icon::Collapse, true)
        .on_hover_text("Collapse the time bar")
        .clicked()
    {
        *toggle = true;
    }
    if icon_button(ui, Icon::Back, view.position > 0)
        .on_hover_text("One frame back (←)")
        .clicked()
    {
        *action = Some(TimeAction::Back);
    }
    play_button(ui, view, action);
    let forward_tip = if view.live || view.position + 1 >= view.positions {
        if view.paused && !view.finished {
            "Compute one more step (→)"
        } else {
            "Follow the simulation again (→)"
        }
    } else {
        "One frame forward (→)"
    };
    if icon_button(ui, Icon::Forward, can_step_forward(view))
        .on_hover_text(forward_tip)
        .on_disabled_hover_text(if view.step_pending {
            "Computing the step…"
        } else {
            "Pause the simulation (cog) to compute single steps with →"
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
    simulation_button(ui, view, action);
}

fn time_label(ui: &mut egui::Ui, view: &TimelineView, text: String) {
    let mut text = egui::RichText::new(text).monospace();
    let paused = view.paused && !view.finished;
    if paused {
        let color = ui.visuals().warn_fg_color;
        text = text.color(if view.loading {
            color.gamma_multiply(0.5)
        } else {
            color
        });
    } else if view.loading {
        text = text.weak();
    }
    let label = ui.label(text);
    if paused {
        label.on_hover_text("The simulation is paused");
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
            live_shown: 9.0,
            rate: VIEW_RATE,
            playing: true,
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
        // Paused on the newest frame: forward goes live.
        view.position = 9;
        assert!(can_step_forward(&view));
        view.live = true;
        view.position = 9;
        assert_eq!(time_text(&view), "9 Myr");
    }

    #[test]
    fn the_walk_goes_on_only_once_its_frame_is_shown() {
        let start = web_time::Instant::now();
        let interval = Duration::from_secs_f64(1.0 / VIEW_RATE);
        let mut walk = Walk::default();
        assert_eq!(walk.wait(start, true), None);
        walk.play();
        // The first frame at once.
        assert_eq!(walk.wait(start, true), Some(Duration::ZERO));
        walk.shown(start);
        // Not before the frame asked for is shown…
        assert_eq!(walk.wait(start + interval, false), None);
        // …then the rest of the interval.
        assert_eq!(
            walk.wait(start + interval / 4, true),
            Some(interval * 3 / 4)
        );
        assert_eq!(walk.wait(start + interval * 2, true), Some(Duration::ZERO));
        walk.stop();
        assert_eq!(walk.wait(start + interval * 2, true), None);
        // A live view keeps the pace.
        assert_eq!(walk.due_in(start + interval / 2), interval / 2);
        // A live view that falls behind walks on one interval after its shown frame.
        walk.fall_behind();
        assert_eq!(walk.wait(start + interval / 2, true), Some(interval / 2));
        walk.stop();
        // Playing again starts at once, not after a stale interval.
        walk.shown(start);
        walk.play();
        assert_eq!(walk.wait(start, true), Some(Duration::ZERO));
    }

    #[test]
    fn the_remembered_state_wins_over_the_screen_width() {
        let time_of = |index: usize| Some(index as f64);
        let view = view(&time_of);
        let ctx = egui::Context::default();
        let input = |width: f32| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 800.0),
            )),
            ..Default::default()
        };
        for (stored, width, open) in [
            (None, 390.0, false),
            (None, 1280.0, true),
            (Some(true), 390.0, true),
            (Some(false), 1280.0, false),
        ] {
            let mut bar = TimeBar::new(stored);
            ctx.run_ui(input(width), |ui| {
                bar.show(ui.ctx(), &view);
            })
            .textures_delta
            .clear();
            assert_eq!(bar.open, Some(open), "{stored:?} at {width}");
            assert_eq!(bar.take_choice(), None);
        }
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
