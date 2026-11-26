//! The parameter panel of the 3D view: shows the command-line options of the running simulation
//! and restarts it with changed ones.
//!
//! The fields are generated from the clap definition of [`Cli`], so the panel, the command line
//! and the URL query of the web version offer the same options under the same names, with the
//! same defaults and the same validation: the panel turns its fields back into command-line
//! arguments and parses them with clap, then checks them like [`crate::build_planet`] does
//! ([`Cli::validate`]).
//!
//! On the web, the default of the subdivisions is the guess for the device rather than clap's
//! (see `crate::device`): the fields show it, "Defaults" returns to it, and the arguments leave
//! it out like any default, so that the page URL only names subdivisions the user chose. The
//! simulation itself always gets them ([`launch_from`]).
//!
//! The seed has no default in the panel: the 3D view starts with a random seed where none is
//! given ([`with_random_seed`]), and the panel always shows the seed of the running simulation so
//! that its planet can be made again. An empty seed field (the hint says "random", "Defaults"
//! empties it) restarts with a new random seed.

use clap::{ArgAction, CommandFactory, Parser};
use eframe::egui;

use super::Launch;
use crate::Cli;

/// The long name of the subdivisions option.
const SUBDIVISIONS: &str = "subdivisions";

/// The long name of the seed option.
const SEED: &str = "seed";

/// Random seeds are below this, so that they stay short in the page URL (at most six digits).
const RANDOM_SEEDS: u64 = 1_000_000;

/// Whether the panel hides an option: the 3D view is never headless, and on the web there are no
/// files either, and `web-math` is always on there. The render mode is chosen in the mode
/// switcher, without a restart.
#[cfg(not(target_arch = "wasm32"))]
fn hidden(name: &str) -> bool {
    name == "headless" || name == super::modes::OPTION
}
#[cfg(target_arch = "wasm32")]
fn hidden(name: &str) -> bool {
    crate::query::WEB_UNAVAILABLE.contains(&name)
        || crate::query::WEB_IMPLIED.contains(&name)
        || name == super::modes::OPTION
}

/// The options shown at the top of the panel, in this order; all others follow in the order of
/// the definition under "More options".
const MAIN: &[&str] = &[
    "seed",
    "subdivisions",
    "plates",
    "continental-fraction",
    "initial",
    "duration",
    "dt",
    "biosphere",
    "epoch",
    "no-erosion",
];

/// Screens narrower than this (in points) start with the panel hidden, e.g. phones in portrait.
const NARROW_SCREEN: f32 = 600.0;

/// Largest width of the panel in points.
const MAX_WIDTH: f32 = 360.0;

/// Distance of the panel and its button from the corner of the window, in points.
const MARGIN: f32 = 8.0;

/// What the panel shows about the running simulation.
pub struct SimulationStatus<'a> {
    /// Simulated time of the displayed state, if there is one yet.
    pub time: Option<f64>,
    /// Simulated duration of the running simulation in Myr.
    pub duration: f64,
    /// Why the simulation failed, if it did.
    pub failure: Option<&'a str>,
    /// What the simulation does until its first update (see `banner::starting_text`).
    pub starting: &'a str,
    /// The median time per step in seconds, once known.
    pub step_seconds: Option<f64>,
}

/// How an option is edited.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    /// A flag without a value (a checkbox).
    Flag,
    /// One of a fixed set of values (a drop-down list).
    Choice(Vec<String>),
    /// A value (a text field).
    Text,
    /// A repeatable option (a text box with one value per line).
    Lines,
}

/// One option of the panel.
#[derive(Debug, Clone)]
struct Field {
    /// The long name, as on the command line (`continental-fraction`).
    name: String,
    help: String,
    kind: Kind,
    /// The default value of a [`Kind::Choice`] or [`Kind::Text`] option, if it has one.
    default: Option<String>,
    /// Whether a [`Kind::Flag`] is set.
    flag: bool,
    /// The value of the other kinds; empty means the default or, for an option without a
    /// default, that it is not given.
    text: String,
}

impl Field {
    fn reset(&mut self) {
        self.flag = false;
        self.text = self.default.clone().unwrap_or_default();
    }

    /// Appends the arguments of this field to `args`; nothing if it has its default value.
    fn push_args(&self, args: &mut Vec<String>) {
        match self.kind {
            Kind::Flag => {
                if self.flag {
                    args.push(format!("--{}", self.name));
                }
            }
            Kind::Choice(_) | Kind::Text => {
                let value = self.text.trim();
                if !value.is_empty() && Some(value) != self.default.as_deref() {
                    args.push(format!("--{}={value}", self.name));
                }
            }
            Kind::Lines => {
                for line in self.text.lines().map(str::trim).filter(|l| !l.is_empty()) {
                    args.push(format!("--{}={line}", self.name));
                }
            }
        }
    }
}

/// The parameter panel, in a corner of the view; hidden, only a small button to show it again
/// remains.
pub struct ParameterPanel {
    /// Whether the panel is shown; decided by the screen width on the first frame.
    open: Option<bool>,
    fields: Vec<Field>,
    /// The arguments of the running simulation, as [`ParameterPanel::args`] produces them.
    running: Vec<String>,
    /// The arguments of the fields when they were last validated, and the error, if any.
    checked: Vec<String>,
    error: Option<String>,
    /// The default of the subdivisions if it is not clap's.
    default_subdivisions: Option<u32>,
}

impl ParameterPanel {
    /// A panel with the options of the running simulation, given as command-line arguments
    /// (starting with the program name), and the default of the subdivisions if it is not
    /// clap's.
    pub fn new(args: &[String], default_subdivisions: Option<u32>) -> Self {
        let command = Cli::command();
        let fields = command
            .get_arguments()
            .filter_map(|arg| {
                let name = arg.get_long()?;
                if hidden(name) {
                    return None;
                }
                let kind = match arg.get_action() {
                    ArgAction::SetTrue => Kind::Flag,
                    ArgAction::Append => Kind::Lines,
                    _ => {
                        let values = arg.get_possible_values();
                        if values.is_empty() {
                            Kind::Text
                        } else {
                            Kind::Choice(values.iter().map(|v| v.get_name().to_owned()).collect())
                        }
                    }
                };
                let help = arg
                    .get_long_help()
                    .or(arg.get_help())
                    .map(ToString::to_string)
                    .unwrap_or_default();
                let default = match default_subdivisions {
                    Some(subdivisions) if name == SUBDIVISIONS => Some(subdivisions.to_string()),
                    // Not clap's 0: an empty field means a random seed.
                    _ if name == SEED => None,
                    _ => (kind != Kind::Flag)
                        .then(|| arg.get_default_values().first())
                        .flatten()
                        .map(|value| value.to_string_lossy().into_owned()),
                };
                let mut field = Field {
                    name: name.to_owned(),
                    help,
                    kind,
                    default,
                    flag: false,
                    text: String::new(),
                };
                field.reset();
                Some(field)
            })
            .collect();
        let mut panel = Self {
            open: None,
            fields,
            running: Vec::new(),
            checked: Vec::new(),
            error: None,
            default_subdivisions,
        };
        panel.set_values(args);
        panel.running = panel.args();
        panel
    }

    /// Sets the fields to the options in `args` (starting with the program name); options not
    /// given get their defaults.
    fn set_values(&mut self, args: &[String]) {
        let matches = match Cli::command().try_get_matches_from(args) {
            Ok(matches) => matches,
            Err(error) => {
                self.error = Some(clap_message(&error));
                return;
            }
        };
        let command = Cli::command();
        for field in &mut self.fields {
            field.reset();
            let Some(arg) = command
                .get_arguments()
                .find(|arg| arg.get_long() == Some(field.name.as_str()))
            else {
                continue;
            };
            let id = arg.get_id().as_str();
            match field.kind {
                Kind::Flag => field.flag = matches.get_flag(id),
                Kind::Choice(_) | Kind::Text | Kind::Lines => {
                    // Only given values: clap's defaults are not the panel's for every option.
                    let given =
                        matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine);
                    if let Some(values) = matches.get_raw(id).filter(|_| given) {
                        let values: Vec<_> = values.map(|v| v.to_string_lossy()).collect();
                        field.text = values.join("\n");
                    }
                }
            }
        }
    }

    /// The options of the fields as command-line arguments, starting with the program name:
    /// `--name=value` for options that differ from their default, `--name` for set flags.
    pub fn args(&self) -> Vec<String> {
        let mut args = vec!["world-simulator".to_owned()];
        for field in &self.fields {
            field.push_args(&mut args);
        }
        args
    }

    /// Records that a simulation with `args` was started.
    pub fn set_running(&mut self, args: Vec<String>) {
        self.running = args;
    }

    /// The simulation to start for the hint of the view: the running one with other
    /// subdivisions. The subdivisions field shows them too; other edits in the panel stay
    /// pending.
    pub fn launch_with_subdivisions(&mut self, subdivisions: u32) -> Result<Launch, String> {
        let edited = self.fields.clone();
        let running = self.running.clone();
        self.set_values(&running);
        self.set_subdivisions(subdivisions);
        let args = self.args();
        self.fields = edited;
        self.set_subdivisions(subdivisions);
        launch_from(args, self.default_subdivisions)
    }

    fn set_subdivisions(&mut self, subdivisions: u32) {
        if let Some(field) = self
            .fields
            .iter_mut()
            .find(|field| field.name == SUBDIVISIONS)
        {
            field.text = subdivisions.to_string();
        }
    }

    /// Shows the panel (or the button that shows it) and returns the simulation to start if
    /// the user asked for a restart. The panel stays above `avoid` (the area reserved for the
    /// cell widget in the bottom left corner) where the two could overlap, on narrow screens,
    /// and always above `time_bar` (along the bottom edge).
    /// The button and the hint under it stay clear of `switcher` (the render-mode switcher in
    /// the top left corner): beside it if there is room, else below it. `hint` draws the
    /// step-time hint: under the status line of the panel, or under the button while the panel
    /// is hidden, so that it is always visible.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        status: &SimulationStatus,
        avoid: Option<egui::Rect>,
        time_bar: Option<egui::Rect>,
        switcher: egui::Rect,
        hint: Option<impl FnOnce(&mut egui::Ui)>,
    ) -> Option<Launch> {
        let args = self.args();
        if args != self.checked {
            self.error = validate(&args, self.default_subdivisions).err();
            self.checked = args;
        }

        let screen = ctx.content_rect();
        let open = *self.open.get_or_insert(screen.width() >= NARROW_SCREEN);
        if !open {
            let width = (screen.width() - 4.0 * MARGIN).clamp(100.0, MAX_WIDTH);
            let (top, width) = beside_or_below(screen, switcher, width, MIN_BESIDE);
            egui::Area::new(egui::Id::unique("parameter panel button"))
                .anchor(egui::Align2::RIGHT_TOP, [-MARGIN, top])
                .show(ctx, |ui| {
                    // The area is as wide as this at most (a right-aligned layout fills it),
                    // so that it does not cover the switcher.
                    ui.set_max_width(width);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        if ui
                            .button("⚙ Parameters")
                            .on_hover_text("Show the simulation parameters")
                            .clicked()
                        {
                            self.open = Some(true);
                        }
                        if let Some(hint) = hint {
                            hint(ui);
                        }
                    });
                });
            return None;
        }

        let max_width = (screen.width() - 4.0 * MARGIN).clamp(100.0, MAX_WIDTH);
        // Beside the switcher if the panel fits there, else below it (phones).
        let (top, max_width) = beside_or_below(screen, switcher, max_width, MIN_PANEL_BESIDE);
        let bottom = match avoid {
            Some(avoid) if avoid.right() + MARGIN > screen.right() - MARGIN - max_width => {
                avoid.top().min(screen.bottom())
            }
            _ => screen.bottom(),
        }
        .min(time_bar.map_or(screen.bottom(), |bar| bar.top()));
        let mut open = true;
        let mut launch = None;
        egui::Window::new("Simulation parameters")
            .id(egui::Id::unique("parameter panel"))
            .anchor(egui::Align2::RIGHT_TOP, [-MARGIN, top])
            .open(&mut open)
            .collapsible(false)
            // As large as the contents, but nothing off-screen on small screens: the contents
            // scroll (the title bar and the frame come on top), and long messages wrap.
            .auto_sized()
            .max_width(max_width)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((bottom - screen.top() - top - 9.0 * MARGIN).max(100.0))
                    .show(ui, |ui| launch = self.contents(ui, status, hint));
            });
        self.open = Some(open);
        if launch.is_some() {
            self.close_after_restart(ctx);
        }
        launch
    }

    /// On narrow screens the panel covers most of the view: it closes on a restart (from the
    /// panel or the step-time hint), so that the status banner behind it is seen.
    pub fn close_after_restart(&mut self, ctx: &egui::Context) {
        if ctx.content_rect().width() < NARROW_SCREEN {
            self.open = Some(false);
        }
    }

    fn contents(
        &mut self,
        ui: &mut egui::Ui,
        status: &SimulationStatus,
        hint: Option<impl FnOnce(&mut egui::Ui)>,
    ) -> Option<Launch> {
        match (status.failure, status.time) {
            (Some(failure), _) => {
                ui.colored_label(ui.visuals().error_fg_color, "Simulation failed:");
                ui.colored_label(ui.visuals().error_fg_color, failure);
            }
            (None, Some(time)) => {
                let pace = status
                    .step_seconds
                    .map(|seconds| format!(" · {seconds:.2} s per step"))
                    .unwrap_or_default();
                ui.label(format!("t = {time:.0} of {:.0} Myr{pace}", status.duration));
            }
            (None, None) => {
                ui.label(status.starting);
            }
        }
        if let Some(hint) = hint {
            hint(ui);
        }
        ui.separator();

        let (main, more): (Vec<&mut Field>, Vec<&mut Field>) = self
            .fields
            .iter_mut()
            .partition(|field| MAIN.contains(&field.name.as_str()));
        let mut main = main;
        main.sort_by_key(|field| MAIN.iter().position(|&name| name == field.name));
        field_grid(ui, "parameter panel main", main);
        egui::CollapsingHeader::new("More options")
            .id_salt("parameter panel more")
            .show(ui, |ui| field_grid(ui, "parameter panel more grid", more));

        ui.separator();
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        } else if self.checked != self.running {
            ui.label("Changed; restart to apply.");
        }
        let mut launch = None;
        ui.horizontal(|ui| {
            let restart = ui
                .add_enabled(self.error.is_none(), egui::Button::new("Restart"))
                .on_hover_text("Restart the simulation with these parameters");
            if restart.clicked() {
                match launch_from(self.checked.clone(), self.default_subdivisions) {
                    Ok(new) => {
                        // Shows the seed chosen for an empty field.
                        self.set_values(&new.args);
                        launch = Some(new);
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            let undo = ui
                .add_enabled(
                    self.checked != self.running,
                    egui::Button::new("Undo changes"),
                )
                .on_hover_text("Show the parameters of the running simulation again");
            if undo.clicked() {
                let running = self.running.clone();
                self.set_values(&running);
            }
            if ui
                .button("Defaults")
                .on_hover_text("Set all parameters to their defaults")
                .clicked()
            {
                self.fields.iter_mut().for_each(Field::reset);
            }
        });
        launch
    }
}

/// Narrowest room (points) beside the switcher for the button and the hint of the hidden panel,
/// and for the open panel.
const MIN_BESIDE: f32 = 160.0;
const MIN_PANEL_BESIDE: f32 = 300.0;

/// Where the panel or its button goes, as the distance of its top from the top of the screen,
/// and how wide it (and the hint under the button) may be (at most `width`): beside `switcher`
/// in the top right corner if there are at least `min_beside` points of room, else below it.
fn beside_or_below(
    screen: egui::Rect,
    switcher: egui::Rect,
    width: f32,
    min_beside: f32,
) -> (f32, f32) {
    let room = screen.right() - MARGIN - (switcher.right() + MARGIN);
    if room >= min_beside.min(width) {
        (MARGIN, width.min(room))
    } else {
        (switcher.bottom() - screen.top() + MARGIN, width)
    }
}

/// Shows the fields as a grid of names and editors.
fn field_grid(ui: &mut egui::Ui, id: &str, fields: Vec<&mut Field>) {
    egui::Grid::new(id)
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for field in fields {
                ui.label(&field.name).on_hover_text(&field.help);
                match &field.kind {
                    Kind::Flag => {
                        ui.checkbox(&mut field.flag, "");
                    }
                    Kind::Choice(values) => {
                        egui::ComboBox::from_id_salt(&field.name)
                            .selected_text(field.text.as_str())
                            .show_ui(ui, |ui| {
                                for value in values {
                                    ui.selectable_value(&mut field.text, value.clone(), value);
                                }
                            });
                    }
                    Kind::Text => {
                        ui.add(
                            egui::TextEdit::singleline(&mut field.text)
                                .desired_width(150.0)
                                .hint_text(field.default.as_deref().unwrap_or(
                                    if field.name == SEED {
                                        "random"
                                    } else {
                                        "not set"
                                    },
                                )),
                        );
                    }
                    Kind::Lines => {
                        ui.add(
                            egui::TextEdit::multiline(&mut field.text)
                                .desired_width(150.0)
                                .desired_rows(2)
                                .hint_text("NAME=VALUE, one per line"),
                        );
                    }
                }
                ui.end_row();
            }
        });
}

/// Whether the command-line arguments `args` (after the program name, as `--name=value` or
/// `--name`) give the option `name`.
pub fn has_option(args: &[String], name: &str) -> bool {
    args.iter().skip(1).any(|arg| {
        arg.strip_prefix("--")
            .and_then(|arg| arg.strip_prefix(name))
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('='))
    })
}

/// `args` with `--name=value` added after the program name, unless they give `name` already:
/// a default that differs from clap's (the subdivisions guessed for the device on the web).
pub fn with_default(args: &[String], name: &str, value: &str) -> Vec<String> {
    let mut args = args.to_vec();
    if !has_option(&args, name) {
        args.insert(1.min(args.len()), format!("--{name}={value}"));
    }
    args
}

/// `args` with a random `--seed` added after the program name, unless they give one: the 3D
/// view starts a new planet each time it is opened without a seed. The arguments name the seed,
/// so the panel and the page URL show it and the planet can be made again. Headless runs keep
/// clap's default seed 0 and stay reproducible.
pub fn with_random_seed(args: &[String]) -> Vec<String> {
    if has_option(args, SEED) {
        return args.to_vec();
    }
    with_default(args, SEED, &random_seed().to_string())
}

/// A random seed below [`RANDOM_SEEDS`], from the random keys of the standard library's hash
/// maps (the operating system's entropy).
#[cfg(not(target_arch = "wasm32"))]
#[expect(
    clippy::disallowed_types,
    reason = "the seed is meant to be random; the simulation itself stays deterministic"
)]
fn random_seed() -> u64 {
    use std::hash::BuildHasher;
    std::collections::hash_map::RandomState::new().hash_one(0_u8) % RANDOM_SEEDS
}

/// A random seed below [`RANDOM_SEEDS`] from the browser (the standard library has no entropy
/// on the web).
#[cfg(target_arch = "wasm32")]
fn random_seed() -> u64 {
    // `Math.random` is in [0, 1), so the product is below `RANDOM_SEEDS`.
    (js_sys::Math::random() * RANDOM_SEEDS as f64) as u64
}

/// Parses and checks command-line arguments (starting with the program name), with
/// `default_subdivisions` where they give none and a random seed if they give none. The
/// arguments of the launch are the given ones (the page URL) with that seed; its options also
/// have the subdivisions.
fn launch_from(args: Vec<String>, default_subdivisions: Option<u32>) -> Result<Launch, String> {
    let args = with_random_seed(&args);
    let complete = match default_subdivisions {
        Some(subdivisions) => with_default(&args, SUBDIVISIONS, &subdivisions.to_string()),
        None => args.clone(),
    };
    let cli = Cli::try_parse_from(&complete).map_err(|error| clap_message(&error))?;
    cli.validate().map_err(|error| format!("{error:#}"))?;
    Ok(Launch { cli, args })
}

fn validate(args: &[String], default_subdivisions: Option<u32>) -> Result<(), String> {
    launch_from(args.to_vec(), default_subdivisions).map(|_| ())
}

/// The message of a clap error without the usage hint.
fn clap_message(error: &clap::Error) -> String {
    let rendered = error.render().to_string();
    let message = rendered.split("\n\n").next().unwrap_or(&rendered).trim();
    message
        .strip_prefix("error: ")
        .unwrap_or(message)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No step-time hint.
    const NO_HINT: Option<fn(&mut egui::Ui)> = None;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|&arg| arg.to_owned()).collect()
    }

    fn field<'a>(panel: &'a mut ParameterPanel, name: &str) -> &'a mut Field {
        panel
            .fields
            .iter_mut()
            .find(|field| field.name == name)
            .unwrap()
    }

    #[test]
    fn every_option_except_the_hidden_ones_has_a_field() {
        let panel = ParameterPanel::new(&strings(&["world-simulator"]), None);
        let names: Vec<&str> = panel.fields.iter().map(|f| f.name.as_str()).collect();
        for arg in Cli::command().get_arguments() {
            let name = arg.get_long().unwrap();
            assert_eq!(names.contains(&name), !hidden(name), "{name}");
        }
        for name in MAIN {
            assert!(names.contains(name), "{name}");
        }
        let kinds = |name: &str| {
            panel
                .fields
                .iter()
                .find(|f| f.name == name)
                .unwrap()
                .kind
                .clone()
        };
        assert_eq!(kinds("seed"), Kind::Text);
        assert_eq!(kinds("no-erosion"), Kind::Flag);
        assert_eq!(kinds("set"), Kind::Lines);
        assert!(
            matches!(kinds("biosphere"), Kind::Choice(values) if values.contains(&"abiotic".to_owned()))
        );
    }

    #[test]
    fn defaults_give_no_arguments() {
        let panel = ParameterPanel::new(&strings(&["world-simulator"]), None);
        assert_eq!(panel.args(), strings(&["world-simulator"]));
        assert_eq!(panel.running, panel.args());
    }

    #[test]
    fn command_line_values_appear_in_the_fields_and_round_trip() {
        let given = strings(&[
            "world-simulator",
            "--seed",
            "3",
            "--no-erosion",
            "--biosphere=modern",
            "--set",
            "basal_drag=8e14",
            "--set=climate.x=-1",
            "--epoch=-0",
            "--co2",
            "2",
        ]);
        let mut panel = ParameterPanel::new(&given, None);
        assert_eq!(field(&mut panel, "seed").text, "3");
        assert!(field(&mut panel, "no-erosion").flag);
        assert_eq!(field(&mut panel, "biosphere").text, "modern");
        assert_eq!(
            field(&mut panel, "set").text,
            "basal_drag=8e14\nclimate.x=-1"
        );
        let args = panel.args();
        assert_eq!(
            args,
            strings(&[
                "world-simulator",
                "--seed=3",
                "--no-erosion",
                "--biosphere=modern",
                "--epoch=-0",
                "--co2=2",
                "--set=basal_drag=8e14",
                "--set=climate.x=-1",
            ])
        );
        // The arguments of the panel mean the same as the given ones.
        let original = Cli::try_parse_from(&given).unwrap();
        let parsed = Cli::try_parse_from(&args).unwrap();
        assert_eq!(format!("{original:?}"), format!("{parsed:?}"));
    }

    #[test]
    fn edits_become_arguments() {
        let mut panel = ParameterPanel::new(&strings(&["world-simulator", "--plates=8"]), None);
        field(&mut panel, "seed").text = " 42 ".to_owned();
        field(&mut panel, "plates").text = String::new();
        field(&mut panel, "set").text = "\n a=1 \n\nb=2\n".to_owned();
        assert_eq!(
            panel.args(),
            strings(&["world-simulator", "--seed=42", "--set=a=1", "--set=b=2"])
        );
        let launch = launch_from(
            strings(&["world-simulator", "--seed=42", "--duration=10"]),
            None,
        )
        .unwrap();
        assert_eq!(launch.cli.seed, 42);
        assert_eq!(launch.cli.duration, 10.0);
        panel.fields.iter_mut().for_each(Field::reset);
        assert_eq!(panel.args(), strings(&["world-simulator"]));
    }

    #[test]
    fn a_missing_seed_is_random_and_named_in_the_arguments() {
        let given = strings(&["world-simulator", "--seed", "5", "--plates=8"]);
        assert_eq!(with_random_seed(&given), given);
        let seeds: Vec<u64> = (0..20)
            .map(|_| {
                let args = with_random_seed(&strings(&["world-simulator", "--plates=8"]));
                assert_eq!(args.len(), 3, "{args:?}");
                assert_eq!(args[2], "--plates=8");
                let seed = args[1].strip_prefix("--seed=").unwrap().parse().unwrap();
                assert!(seed < RANDOM_SEEDS);
                seed
            })
            .collect();
        assert!(seeds.iter().any(|&seed| seed != seeds[0]), "{seeds:?}");
        // A launch from the panel without a seed gets one too.
        let launch = launch_from(strings(&["world-simulator"]), None).unwrap();
        assert!(has_option(&launch.args, SEED));
        assert_eq!(
            launch.args,
            strings(&["world-simulator", &format!("--seed={}", launch.cli.seed)])
        );
    }

    #[test]
    fn the_seed_field_has_no_default() {
        let mut panel = ParameterPanel::new(&strings(&["world-simulator", "--seed=0"]), None);
        // Seed 0 is a seed like any other, not the default.
        assert_eq!(field(&mut panel, "seed").text, "0");
        assert_eq!(panel.args(), strings(&["world-simulator", "--seed=0"]));
        panel.fields.iter_mut().for_each(Field::reset);
        assert_eq!(field(&mut panel, "seed").text, "");
        assert_eq!(panel.args(), strings(&["world-simulator"]));
        // The fields show the seed chosen at a restart, as the arguments of the panel.
        let launch = launch_from(panel.args(), None).unwrap();
        panel.set_values(&launch.args);
        assert_eq!(field(&mut panel, "seed").text, launch.cli.seed.to_string());
        assert_eq!(panel.args(), launch.args);
    }

    #[test]
    fn invalid_values_are_reported() {
        let error = |args: &[&str]| validate(&strings(args), None).unwrap_err();
        let message = error(&["world-simulator", "--seed=abc"]);
        assert!(message.contains("--seed"), "{message}");
        assert!(!message.contains("Usage"), "{message}");
        assert!(!message.starts_with("error:"), "{message}");
        assert!(error(&["world-simulator", "--subdivisions=9"]).contains("subdivisions"));
        assert!(error(&["world-simulator", "--dt=0"]).contains("time step"));
        assert!(error(&["world-simulator", "--subdivisions=0", "--plates=13"]).contains("cells"));
        assert!(
            validate(
                &strings(&["world-simulator", "--subdivisions=0", "--plates=12"]),
                None
            )
            .is_ok()
        );
        assert!(error(&["world-simulator", "--epoch=-1"]).contains("epoch"));
        assert!(error(&["world-simulator", "--set=nonexistent=1"]).contains("nonexistent"));
        assert!(error(&["world-simulator", "--set=basal_drag"]).contains("NAME=VALUE"));
        assert!(
            validate(
                &strings(&["world-simulator", "--set=basal_drag=8e14"]),
                None
            )
            .is_ok()
        );
    }

    #[test]
    fn the_panel_shows_no_error_for_valid_values_and_one_for_invalid_ones() {
        // Runs one frame; the textures it produced are not needed.
        fn run(ctx: &egui::Context, input: egui::RawInput, ui: impl FnMut(&mut egui::Ui)) {
            ctx.run_ui(input, ui).textures_delta.clear();
        }
        let ctx = egui::Context::default();
        let mut panel = ParameterPanel::new(&strings(&["world-simulator"]), None);
        let status = SimulationStatus {
            time: Some(10.0),
            duration: 500.0,
            failure: None,
            starting: "Starting the simulation…",
            step_seconds: Some(0.4),
        };
        let mut launch = None;
        run(&ctx, egui::RawInput::default(), |ui| {
            launch = panel.show(ui.ctx(), &status, None, None, egui::Rect::NOTHING, NO_HINT);
        });
        assert!(launch.is_none());
        assert!(panel.error.is_none());
        field(&mut panel, "plates").text = "0".to_owned();
        run(&ctx, egui::RawInput::default(), |ui| {
            panel.show(ui.ctx(), &status, None, None, egui::Rect::NOTHING, NO_HINT);
        });
        assert!(panel.error.as_deref().unwrap().contains("plate"));
        // Hidden, the panel keeps its values.
        panel.open = Some(false);
        run(&ctx, egui::RawInput::default(), |ui| {
            panel.show(ui.ctx(), &status, None, None, egui::Rect::NOTHING, NO_HINT);
        });
        assert_eq!(field(&mut panel, "plates").text, "0");

        // On a phone in portrait, the panel starts hidden.
        let ctx = egui::Context::default();
        let mut panel = ParameterPanel::new(&strings(&["world-simulator"]), None);
        let phone = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(390.0, 844.0),
            )),
            ..Default::default()
        };
        run(&ctx, phone, |ui| {
            panel.show(ui.ctx(), &status, None, None, egui::Rect::NOTHING, NO_HINT);
        });
        assert_eq!(panel.open, Some(false));
    }

    #[test]
    fn another_default_of_the_subdivisions_is_shown_and_left_out_of_the_arguments() {
        let clap_default = Cli::try_parse_from(["world-simulator"])
            .unwrap()
            .subdivisions;
        let guess = if clap_default == 6 { 5 } else { 6 };
        let mut panel =
            ParameterPanel::new(&strings(&["world-simulator", "--seed=3"]), Some(guess));
        assert_eq!(field(&mut panel, "subdivisions").text, guess.to_string());
        assert_eq!(panel.args(), strings(&["world-simulator", "--seed=3"]));
        // The simulation gets them, the arguments (the URL) do not name them.
        let launch = launch_from(panel.args(), Some(guess)).unwrap();
        assert_eq!(launch.cli.subdivisions, guess);
        assert_eq!(launch.args, strings(&["world-simulator", "--seed=3"]));
        // Chosen values other than the default are named, also clap's default.
        field(&mut panel, "subdivisions").text = clap_default.to_string();
        assert!(
            panel
                .args()
                .contains(&format!("--subdivisions={clap_default}"))
        );
        // "Defaults" returns to the guess.
        panel.fields.iter_mut().for_each(Field::reset);
        assert_eq!(field(&mut panel, "subdivisions").text, guess.to_string());
        // Given subdivisions win over the default.
        let launch = launch_from(
            strings(&["world-simulator", "--subdivisions=4"]),
            Some(guess),
        );
        assert_eq!(launch.unwrap().cli.subdivisions, 4);
    }

    #[test]
    fn the_hint_restarts_the_running_simulation_with_other_subdivisions() {
        let running = strings(&["world-simulator", "--seed=3"]);
        let mut panel = ParameterPanel::new(&running, Some(6));
        // A pending edit, not part of the restart.
        field(&mut panel, "plates").text = "8".to_owned();
        let launch = panel.launch_with_subdivisions(5).unwrap();
        assert_eq!(launch.cli.subdivisions, 5);
        assert_eq!(launch.cli.plates, 12);
        assert_eq!(
            launch.args,
            strings(&["world-simulator", "--seed=3", "--subdivisions=5"])
        );
        // The fields show the new subdivisions and keep the edit.
        assert_eq!(field(&mut panel, "subdivisions").text, "5");
        assert_eq!(field(&mut panel, "plates").text, "8");
        // Back to the default: left out of the arguments again.
        panel.set_running(launch.args);
        let launch = panel.launch_with_subdivisions(6).unwrap();
        assert_eq!(launch.cli.subdivisions, 6);
        assert_eq!(launch.args, strings(&["world-simulator", "--seed=3"]));
    }

    #[test]
    fn the_hidden_panel_keeps_clear_of_the_switcher() {
        let switcher = egui::Rect::from_min_max(egui::pos2(8.0, 8.0), egui::pos2(250.0, 60.0));
        // Wide screen: in the corner, as wide as allowed.
        let wide = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
        assert_eq!(
            beside_or_below(wide, switcher, 360.0, MIN_BESIDE),
            (MARGIN, 360.0)
        );
        // A phone with room beside the switcher: in the corner, narrower.
        let phone = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(430.0, 800.0));
        let (top, width) = beside_or_below(phone, switcher, 360.0, MIN_BESIDE);
        assert_eq!(top, MARGIN);
        assert_eq!(width, 430.0 - MARGIN - 258.0);
        assert!(phone.right() - MARGIN - width >= switcher.right());
        // Too little room: below the switcher.
        let small = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 640.0));
        assert_eq!(
            beside_or_below(small, switcher, 328.0, MIN_BESIDE),
            (68.0, 328.0)
        );
        // The open panel needs more room: below the switcher on the phone.
        assert_eq!(
            beside_or_below(phone, switcher, 360.0, MIN_PANEL_BESIDE),
            (68.0, 360.0)
        );
    }

    #[test]
    fn options_are_found_and_defaults_added_only_where_missing() {
        let args = strings(&[
            "world-simulator",
            "--seed=3",
            "--subdivisions=4",
            "--no-erosion",
        ]);
        assert!(has_option(&args, "subdivisions"));
        assert!(has_option(&args, "no-erosion"));
        assert!(!has_option(&args, "plates"));
        // A prefix of another option's name does not count.
        assert!(!has_option(&args, "subdiv"));
        assert!(!has_option(&args, "no"));
        assert_eq!(with_default(&args, "subdivisions", "6"), args);
        let args = strings(&["world-simulator", "--seed=3"]);
        assert_eq!(
            with_default(&args, "subdivisions", "6"),
            strings(&["world-simulator", "--subdivisions=6", "--seed=3"])
        );
    }

    #[test]
    fn nothing_is_pending_after_a_hint_restart() {
        let mut panel = ParameterPanel::new(&strings(&["world-simulator", "--seed=3"]), Some(6));
        let launch = panel.launch_with_subdivisions(5).unwrap();
        panel.set_running(launch.args);
        // The panel compares its arguments with the running ones for "Changed".
        assert_eq!(panel.args(), panel.running);
    }
}
