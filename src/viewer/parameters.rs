//! The parameter panel of the 3D view: shows the command-line options of the running simulation
//! and restarts it with changed ones.
//!
//! The fields are generated from the clap definition of [`Cli`], so the panel, the command line
//! and the URL query of the web version offer the same options under the same names, with the
//! same defaults and the same validation: the panel turns its fields back into command-line
//! arguments and parses them with clap, then checks them like [`crate::build_planet`] does
//! ([`Cli::validate`]).

use clap::{ArgAction, CommandFactory, Parser};
use eframe::egui;

use super::Launch;
use crate::Cli;

/// Whether the panel hides an option: the 3D view is never headless, and on the web there are no
/// files either, and `web-math` is always on there.
#[cfg(not(target_arch = "wasm32"))]
fn hidden(name: &str) -> bool {
    name == "headless"
}
#[cfg(target_arch = "wasm32")]
fn hidden(name: &str) -> bool {
    crate::query::WEB_UNAVAILABLE.contains(&name) || crate::query::WEB_IMPLIED.contains(&name)
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
}

impl ParameterPanel {
    /// A panel with the options of the running simulation, given as command-line arguments
    /// (starting with the program name).
    pub fn new(args: &[String]) -> Self {
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
                let default = (kind != Kind::Flag)
                    .then(|| arg.get_default_values().first())
                    .flatten()
                    .map(|value| value.to_string_lossy().into_owned());
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
                    if let Some(values) = matches.get_raw(id) {
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

    /// Shows the panel (or the button that shows it) and returns the simulation to start if
    /// the user asked for a restart. The panel stays above `avoid` (the area reserved for the
    /// cell widget in the bottom left corner) where the two could overlap, on narrow screens.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        status: &SimulationStatus,
        avoid: Option<egui::Rect>,
    ) -> Option<Launch> {
        let args = self.args();
        if args != self.checked {
            self.error = validate(&args).err();
            self.checked = args;
        }

        let screen = ctx.content_rect();
        let open = *self.open.get_or_insert(screen.width() >= NARROW_SCREEN);
        if !open {
            egui::Area::new(egui::Id::unique("parameter panel button"))
                .anchor(egui::Align2::RIGHT_TOP, [-MARGIN, MARGIN])
                .show(ctx, |ui| {
                    if ui
                        .button("⚙ Parameters")
                        .on_hover_text("Show the simulation parameters")
                        .clicked()
                    {
                        self.open = Some(true);
                    }
                });
            return None;
        }

        let max_width = (screen.width() - 4.0 * MARGIN).clamp(100.0, MAX_WIDTH);
        let bottom = match avoid {
            Some(avoid) if avoid.right() + MARGIN > screen.right() - MARGIN - max_width => {
                avoid.top().min(screen.bottom())
            }
            _ => screen.bottom(),
        };
        let mut open = true;
        let mut launch = None;
        egui::Window::new("Simulation parameters")
            .id(egui::Id::unique("parameter panel"))
            .anchor(egui::Align2::RIGHT_TOP, [-MARGIN, MARGIN])
            .open(&mut open)
            .collapsible(false)
            // As large as the contents, but nothing off-screen on small screens: the contents
            // scroll (the title bar and the frame come on top), and long messages wrap.
            .auto_sized()
            .max_width(max_width)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((bottom - screen.top() - 10.0 * MARGIN).max(100.0))
                    .show(ui, |ui| launch = self.contents(ui, status));
            });
        self.open = Some(open);
        launch
    }

    fn contents(&mut self, ui: &mut egui::Ui, status: &SimulationStatus) -> Option<Launch> {
        match (status.failure, status.time) {
            (Some(failure), _) => {
                ui.colored_label(ui.visuals().error_fg_color, "Simulation failed:");
                ui.colored_label(ui.visuals().error_fg_color, failure);
            }
            (None, Some(time)) => {
                ui.label(format!("t = {time:.0} of {:.0} Myr", status.duration));
            }
            (None, None) => {
                ui.label("Starting the simulation…");
            }
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
                match launch_from(self.checked.clone()) {
                    Ok(new) => launch = Some(new),
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
                                .hint_text(field.default.as_deref().unwrap_or("not set")),
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

/// Parses and checks command-line arguments (starting with the program name).
fn launch_from(args: Vec<String>) -> Result<Launch, String> {
    let cli = Cli::try_parse_from(&args).map_err(|error| clap_message(&error))?;
    cli.validate().map_err(|error| format!("{error:#}"))?;
    Ok(Launch { cli, args })
}

fn validate(args: &[String]) -> Result<(), String> {
    launch_from(args.to_vec()).map(|_| ())
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
        let panel = ParameterPanel::new(&strings(&["world-simulator"]));
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
        let panel = ParameterPanel::new(&strings(&["world-simulator"]));
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
            "--biosphere=abiotic",
            "--set",
            "basal_drag=8e14",
            "--set=climate.x=-1",
            "--epoch=-0",
            "--co2",
            "2",
        ]);
        let mut panel = ParameterPanel::new(&given);
        assert_eq!(field(&mut panel, "seed").text, "3");
        assert!(field(&mut panel, "no-erosion").flag);
        assert_eq!(field(&mut panel, "biosphere").text, "abiotic");
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
                "--biosphere=abiotic",
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
        let mut panel = ParameterPanel::new(&strings(&["world-simulator", "--plates=8"]));
        field(&mut panel, "seed").text = " 42 ".to_owned();
        field(&mut panel, "plates").text = String::new();
        field(&mut panel, "set").text = "\n a=1 \n\nb=2\n".to_owned();
        assert_eq!(
            panel.args(),
            strings(&["world-simulator", "--seed=42", "--set=a=1", "--set=b=2"])
        );
        let launch =
            launch_from(strings(&["world-simulator", "--seed=42", "--duration=10"])).unwrap();
        assert_eq!(launch.cli.seed, 42);
        assert_eq!(launch.cli.duration, 10.0);
        panel.fields.iter_mut().for_each(Field::reset);
        assert_eq!(panel.args(), strings(&["world-simulator"]));
    }

    #[test]
    fn invalid_values_are_reported() {
        let error = |args: &[&str]| validate(&strings(args)).unwrap_err();
        let message = error(&["world-simulator", "--seed=abc"]);
        assert!(message.contains("--seed"), "{message}");
        assert!(!message.contains("Usage"), "{message}");
        assert!(!message.starts_with("error:"), "{message}");
        assert!(error(&["world-simulator", "--subdivisions=9"]).contains("subdivisions"));
        assert!(error(&["world-simulator", "--dt=0"]).contains("time step"));
        assert!(error(&["world-simulator", "--subdivisions=0", "--plates=13"]).contains("cells"));
        assert!(
            validate(&strings(&[
                "world-simulator",
                "--subdivisions=0",
                "--plates=12"
            ]))
            .is_ok()
        );
        assert!(error(&["world-simulator", "--epoch=-1"]).contains("epoch"));
        assert!(error(&["world-simulator", "--set=nonexistent=1"]).contains("nonexistent"));
        assert!(error(&["world-simulator", "--set=basal_drag"]).contains("NAME=VALUE"));
        assert!(validate(&strings(&["world-simulator", "--set=basal_drag=8e14"])).is_ok());
    }

    #[test]
    fn the_panel_shows_no_error_for_valid_values_and_one_for_invalid_ones() {
        // Runs one frame; the textures it produced are not needed.
        fn run(ctx: &egui::Context, input: egui::RawInput, ui: impl FnMut(&mut egui::Ui)) {
            ctx.run_ui(input, ui).textures_delta.clear();
        }
        let ctx = egui::Context::default();
        let mut panel = ParameterPanel::new(&strings(&["world-simulator"]));
        let status = SimulationStatus {
            time: Some(10.0),
            duration: 500.0,
            failure: None,
        };
        let mut launch = None;
        run(&ctx, egui::RawInput::default(), |ui| {
            launch = panel.show(ui.ctx(), &status, None);
        });
        assert!(launch.is_none());
        assert!(panel.error.is_none());
        field(&mut panel, "plates").text = "0".to_owned();
        run(&ctx, egui::RawInput::default(), |ui| {
            panel.show(ui.ctx(), &status, None);
        });
        assert!(panel.error.as_deref().unwrap().contains("plate"));
        // Hidden, the panel keeps its values.
        panel.open = Some(false);
        run(&ctx, egui::RawInput::default(), |ui| {
            panel.show(ui.ctx(), &status, None);
        });
        assert_eq!(field(&mut panel, "plates").text, "0");

        // On a phone in portrait, the panel starts hidden.
        let ctx = egui::Context::default();
        let mut panel = ParameterPanel::new(&strings(&["world-simulator"]));
        let phone = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(390.0, 844.0),
            )),
            ..Default::default()
        };
        run(&ctx, phone, |ui| {
            panel.show(ui.ctx(), &status, None);
        });
        assert_eq!(panel.open, Some(false));
    }
}
