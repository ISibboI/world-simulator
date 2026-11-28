//! The mode switcher in the top left corner of the 3D view: a drop-down list of the render
//! modes and the legend of the active one (colour bar, swatches, arrow scale), which can be
//! hidden, with the settings of the mode (see [`super::RenderMode::options`]) above the legend,
//! and for the realistic mode a line about the sun of the shown time.
//! It is free there: the parameter panel is in the top right corner, the cell widget
//! in the bottom left, and the banner across the middle. Like those, the legend starts hidden
//! on narrow screens (phones), where only the drop-down list remains.

use eframe::egui;

use super::{Legend, OptionKind, OptionValues, RenderModeId, Swatch, SwatchShape};

/// Screens narrower than this (in points) start with the legend hidden, as the other panels.
const NARROW_SCREEN: f32 = 600.0;
/// Largest width of the legend in points.
const MAX_WIDTH: f32 = 260.0;
/// Distance from the corner of the window, in points.
const MARGIN: f32 = 8.0;
/// The legend scrolls when it would reach the cell widget, but is at least this share of the
/// screen height high.
const MIN_LEGEND_HEIGHT: f32 = 0.4;
/// Height of the colour bar and size of the swatches, in multiples of the body text size.
const BAR_HEIGHT: f32 = 0.9;

/// What the switcher needs to know about the view.
pub struct SwitcherView<'a> {
    /// The legend of the active mode.
    pub legend: &'a Legend,
    /// Points per planet radius on the planet's front, to draw the arrow scale at the size of
    /// the arrows.
    pub points_per_radius: f32,
    /// Whether the data of the active mode has not arrived yet.
    pub waiting: bool,
    /// The top of what lies below the switcher in the same corner (the cell widget): the
    /// legend scrolls rather than reach it.
    pub bottom: f32,
    /// A description of the sun, shown under the options of the realistic mode.
    pub sun: Option<String>,
}

/// The switcher's state.
pub struct ModeSwitcher {
    /// Whether the legend is shown; decided by the screen width on the first frame.
    legend_open: Option<bool>,
    /// The values of every mode's options, in the order of [`RenderModeId::all`]; a mode keeps
    /// them while another one is shown.
    options: Vec<OptionValues>,
}

impl Default for ModeSwitcher {
    fn default() -> Self {
        Self {
            legend_open: None,
            options: RenderModeId::all()
                .iter()
                .map(|mode| OptionValues::defaults(mode.mode().options()))
                .collect(),
        }
    }
}

impl ModeSwitcher {
    /// The values of the options of `mode`.
    pub fn options(&self, mode: RenderModeId) -> &OptionValues {
        static NONE: OptionValues = OptionValues(Vec::new());
        RenderModeId::all()
            .iter()
            .position(|&other| other == mode)
            .and_then(|index| self.options.get(index))
            .unwrap_or(&NONE)
    }

    /// Shows the switcher; changes `mode` if the user chose another one. Returns whether that
    /// happened or the user changed a setting of the mode, and the area the switcher covers
    /// (the parameter panel's button keeps clear of it on narrow screens).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        mode: &mut RenderModeId,
        view: &SwitcherView,
    ) -> (bool, egui::Rect) {
        let screen = ctx.content_rect();
        let legend_open = self
            .legend_open
            .get_or_insert(screen.width() >= NARROW_SCREEN);
        let width = (screen.width() - 4.0 * MARGIN).clamp(100.0, MAX_WIDTH);
        let mut changed = false;
        let mut controls = |ui: &mut egui::Ui, mode: &mut RenderModeId, legend_open: &mut bool| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("render mode")
                    .selected_text(mode.mode().label())
                    .show_ui(ui, |ui| {
                        for &choice in RenderModeId::all() {
                            if ui
                                .selectable_value(mode, choice, choice.mode().label())
                                .changed()
                            {
                                changed = true;
                            }
                        }
                    })
                    .response
                    .on_hover_text("What the planet shows");
                // A framed button (a toggle has no frame while off, which is hard to see over
                // the dark background).
                if ui
                    .add(egui::Button::new("Legend").selected(*legend_open))
                    .on_hover_text("Show or hide the legend")
                    .clicked()
                {
                    *legend_open = !*legend_open;
                }
            });
        };
        let mut options_changed = false;
        let all_options = &mut self.options;
        let response = egui::Area::new(egui::Id::unique("render mode switcher"))
            .anchor(egui::Align2::LEFT_TOP, [MARGIN, MARGIN])
            .show(ctx, |ui| {
                if *legend_open {
                    egui::Frame::window(ui.style()).show(ui, |ui| {
                        ui.set_max_width(width);
                        controls(ui, mode, legend_open);
                        ui.separator();
                        // The mode as chosen in this frame.
                        if let Some(values) = RenderModeId::all()
                            .iter()
                            .position(|&other| other == *mode)
                            .and_then(|index| all_options.get_mut(index))
                        {
                            options_changed = option_controls(ui, *mode, values);
                        }
                        if let Some(sun) = &view.sun {
                            ui.add(egui::Label::new(egui::RichText::new(sun).small()).wrap());
                            ui.separator();
                        }
                        // Long legends scroll (phones in landscape, small windows).
                        // Above the cell widget, but at least a share of the screen (they
                        // may overlap on short screens; either can be hidden).
                        let room =
                            view.bottom.min(screen.bottom()) - 2.0 * MARGIN - ui.cursor().top();
                        let least = MIN_LEGEND_HEIGHT * screen.height();
                        egui::ScrollArea::vertical()
                            .max_height(room.max(least))
                            // Also on the first frame after the legend opened (the area has
                            // the size of the controls alone then).
                            .min_scrolled_height(least)
                            .show(ui, |ui| legend(ui, view.legend, view, width));
                    });
                } else {
                    // Hidden, only the controls remain, like the buttons of the other panels.
                    controls(ui, mode, legend_open);
                }
            });
        (changed || options_changed, response.response.rect)
    }
}

/// The controls of the options of `mode`; returns whether a value changed.
fn option_controls(ui: &mut egui::Ui, mode: RenderModeId, values: &mut OptionValues) -> bool {
    let options = mode.mode().options();
    if options.is_empty() {
        return false;
    }
    let mut changed = false;
    for (index, option) in options.iter().enumerate() {
        match option.kind {
            OptionKind::Choice(choices) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(option.label).on_hover_text(option.hover);
                    for (value, &choice) in choices.iter().enumerate() {
                        let value = value as u8;
                        if ui
                            .selectable_label(values.get(index) == value, choice)
                            .on_hover_text(option.hover)
                            .clicked()
                            && values.get(index) != value
                        {
                            values.set(index, value);
                            changed = true;
                        }
                    }
                });
            }
            OptionKind::Switch { .. } => {
                let mut on = values.is_on(index);
                if ui
                    .checkbox(&mut on, option.label)
                    .on_hover_text(option.hover)
                    .changed()
                {
                    values.set(index, u8::from(on));
                    changed = true;
                }
            }
        }
    }
    ui.separator();
    changed
}

/// Draws a legend `width` points wide.
fn legend(ui: &mut egui::Ui, legend: &Legend, view: &SwitcherView, width: f32) {
    ui.set_width(width);
    let text_size = egui::TextStyle::Body.resolve(ui.style()).size;
    let bar_height = BAR_HEIGHT * text_size;
    ui.add(egui::Label::new(egui::RichText::new(legend.description.as_ref()).small()).wrap());
    if let Some(scale) = &legend.scale {
        ui.label(scale.title.as_ref());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, bar_height), egui::Sense::hover());
        let (low, high) = scale.range();
        let x = |value: f64| {
            rect.left()
                + ((value - low) / (high - low).max(f64::MIN_POSITIVE)) as f32 * rect.width()
        };
        let mut mesh = egui::Mesh::default();
        for &(value, [r, g, b]) in &scale.stops {
            let color = egui::Color32::from_rgb(r, g, b);
            mesh.colored_vertex(egui::pos2(x(value), rect.top()), color);
            mesh.colored_vertex(egui::pos2(x(value), rect.bottom()), color);
        }
        for index in 1..scale.stops.len() as u32 {
            let (a, b) = (2 * index - 2, 2 * index - 1);
            mesh.add_triangle(a, b, a + 2);
            mesh.add_triangle(b, a + 3, a + 2);
        }
        let painter = ui.painter();
        painter.add(egui::Shape::mesh(mesh));
        painter.rect_stroke(
            rect,
            0.0,
            ui.visuals().widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Outside,
        );
        // Tick labels under the bar; the outermost ones stay inside it.
        let font = egui::TextStyle::Small.resolve(ui.style());
        let (label_rect, _) =
            ui.allocate_exact_size(egui::vec2(width, font.size * 1.3), egui::Sense::hover());
        for (value, label) in &scale.ticks {
            let position = x(*value);
            let galley = ui.painter().layout_no_wrap(
                label.to_string(),
                font.clone(),
                ui.visuals().text_color(),
            );
            let left = (position - galley.size().x / 2.0)
                .clamp(label_rect.left(), label_rect.right() - galley.size().x);
            ui.painter().line_segment(
                [
                    egui::pos2(position, rect.bottom()),
                    egui::pos2(position, rect.bottom() + 0.25 * font.size),
                ],
                egui::Stroke::new(1.0, ui.visuals().text_color()),
            );
            ui.painter().galley(
                egui::pos2(left, label_rect.top() + 0.2 * font.size),
                galley,
                ui.visuals().text_color(),
            );
        }
    }
    // Filled squares share wrapped rows, palettes and entries get a row each, lines and markers
    // two columns.
    let mut rest = legend.swatches.as_slice();
    let mut grids = 0;
    while let Some(first) = rest.first() {
        let count = match first.shape {
            SwatchShape::Palette(_) | SwatchShape::Entry => 1,
            SwatchShape::Fill => rest
                .iter()
                .take_while(|swatch| swatch.shape == SwatchShape::Fill)
                .count(),
            _ => rest
                .iter()
                .take_while(|swatch| {
                    !matches!(
                        swatch.shape,
                        SwatchShape::Fill | SwatchShape::Palette(_) | SwatchShape::Entry
                    )
                })
                .count(),
        };
        let group = &rest[..count];
        match first.shape {
            SwatchShape::Palette(_) | SwatchShape::Entry => {
                ui.horizontal(|ui| {
                    swatch_symbol(ui, first, bar_height);
                    ui.add(egui::Label::new(first.label.as_ref()).wrap());
                });
            }
            SwatchShape::Fill => {
                ui.horizontal_wrapped(|ui| {
                    let font = egui::TextStyle::Body.resolve(ui.style());
                    for swatch in group {
                        // A square and its label wrap together: a new row if both do not fit
                        // into the rest of this one.
                        let label = ui.painter().layout_no_wrap(
                            swatch.label.to_string(),
                            font.clone(),
                            ui.visuals().text_color(),
                        );
                        let needed =
                            bar_height + 2.0 * ui.spacing().item_spacing.x + label.size().x;
                        let rest = ui.available_size_before_wrap().x;
                        // (Not at the start of a row, where a long label wraps anyway.)
                        if needed > rest && rest < ui.max_rect().width() - 1.0 {
                            ui.end_row();
                        }
                        swatch_symbol(ui, swatch, bar_height);
                        ui.label(swatch.label.as_ref());
                    }
                });
            }
            _ => {
                grids += 1;
                egui::Grid::new(("legend swatches", grids))
                    .num_columns(2)
                    .spacing([0.5 * bar_height, 0.25 * bar_height])
                    .show(ui, |ui| {
                        for (index, swatch) in group.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.3 * bar_height;
                                swatch_symbol(ui, swatch, bar_height);
                                ui.label(swatch.label.as_ref());
                            });
                            if index % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
            }
        }
        rest = &rest[count..];
    }
    for arrows in &legend.arrows {
        if let Some(label) = &arrows.label {
            ui.label(label.as_ref());
        }
        ui.horizontal_wrapped(|ui| {
            for &(magnitude, length, shaft) in &arrows.examples {
                let scale = view.points_per_radius;
                let length = (length as f32 * scale).clamp(4.0, width / 2.0);
                let shaft = (shaft as f32 * scale).clamp(2.0, bar_height / 2.4);
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(length + 4.0, bar_height),
                    egui::Sense::hover(),
                );
                arrow_shape(ui.painter(), rect, length, shaft, arrows.color);
                // No break between the number and the unit.
                ui.label(format!("{magnitude}\u{a0}{}", arrows.unit));
            }
        });
    }
    if view.waiting {
        ui.weak("Waiting for the data of this mode…");
    }
}

/// The symbol of a swatch, `size` points high.
fn swatch_symbol(ui: &mut egui::Ui, swatch: &Swatch, size: f32) {
    let [r, g, b] = swatch.color;
    let color = egui::Color32::from_rgb(r, g, b);
    let frame = ui.visuals().widgets.noninteractive.bg_stroke;
    let dark = egui::Color32::from_gray(15);
    let width = match swatch.shape {
        SwatchShape::Fill | SwatchShape::Entry => size,
        SwatchShape::Palette(ref colors) => size * 0.6 * colors.len() as f32,
        _ => 1.6 * size,
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, size), egui::Sense::hover());
    let painter = ui.painter();
    let line_width = (0.22 * size).max(2.0);
    let line = |painter: &egui::Painter| {
        let y = rect.center().y;
        let ends = [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)];
        // Dark lines (coastlines, isotherms) get a light border to show on the dark panel.
        let border = if u32::from(r) + u32::from(g) + u32::from(b) < 150 {
            egui::Color32::from_gray(220)
        } else {
            dark
        };
        painter.line_segment(ends, egui::Stroke::new(line_width + 2.0, border));
        painter.line_segment(ends, egui::Stroke::new(line_width, color));
    };
    match &swatch.shape {
        SwatchShape::Fill | SwatchShape::Entry => {
            painter.rect_filled(rect, 2.0, color);
            painter.rect_stroke(rect, 2.0, frame, egui::StrokeKind::Outside);
        }
        SwatchShape::Line => line(painter),
        SwatchShape::Teeth => {
            // Teeth above the line, pointing up (towards the upper plate).
            let y = rect.center().y + 0.15 * size;
            let tooth = 0.45 * size;
            let mut x = rect.left() + 0.3 * size;
            while x + tooth <= rect.right() {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        egui::pos2(x, y),
                        egui::pos2(x + tooth, y),
                        egui::pos2(x + tooth / 2.0, y - tooth),
                    ],
                    color,
                    egui::Stroke::new(1.0, dark),
                ));
                x += 1.6 * tooth;
            }
            let ends = [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)];
            painter.line_segment(ends, egui::Stroke::new(line_width + 2.0, dark));
            painter.line_segment(ends, egui::Stroke::new(line_width, color));
        }
        SwatchShape::Triangle => {
            let center = rect.center();
            let half = 0.45 * size;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(center.x - half, center.y + 0.8 * half),
                    egui::pos2(center.x + half, center.y + 0.8 * half),
                    egui::pos2(center.x, center.y - half),
                ],
                color,
                egui::Stroke::new(1.5, egui::Color32::from_gray(235)),
            ));
        }
        SwatchShape::Dots => {
            painter.rect_filled(rect, 2.0, color);
            let step = size / 3.0;
            for row in 0..3 {
                for column in 0..6 {
                    let x = rect.left() + step * (column as f32 + 0.5);
                    let y = rect.top() + step * (row as f32 + 0.5);
                    if x < rect.right() {
                        painter.circle_filled(egui::pos2(x, y), 0.12 * size, dark);
                    }
                }
            }
            painter.rect_stroke(rect, 2.0, frame, egui::StrokeKind::Outside);
        }
        SwatchShape::Palette(colors) => {
            let step = rect.width() / colors.len().max(1) as f32;
            for (index, &[r, g, b]) in colors.iter().enumerate() {
                let left = rect.left() + step * index as f32;
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(left, rect.top()),
                        egui::pos2(left + step, rect.bottom()),
                    ),
                    0.0,
                    egui::Color32::from_rgb(r, g, b),
                );
            }
            painter.rect_stroke(rect, 2.0, frame, egui::StrokeKind::Outside);
        }
    }
}

/// An arrow pointing right, `length` points long with a shaft `shaft` points wide, in the
/// middle of `rect`, shaped like the arrows on the planet (see `arrows::ArrowStyle`).
fn arrow_shape(
    painter: &egui::Painter,
    rect: egui::Rect,
    length: f32,
    shaft: f32,
    color: [f32; 4],
) {
    let [r, g, b, _] = color.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8);
    let fill = egui::Color32::from_rgb(r, g, b);
    let border = egui::Stroke::new(1.5, egui::Color32::BLACK);
    let y = rect.center().y;
    let start = rect.left() + 2.0;
    let head = 0.4 * length;
    let half_shaft = shaft / 2.0;
    let half_head = 2.4 * half_shaft;
    let points = vec![
        egui::pos2(start, y - half_shaft),
        egui::pos2(start + length - head, y - half_shaft),
        egui::pos2(start + length - head, y - half_head),
        egui::pos2(start + length, y),
        egui::pos2(start + length - head, y + half_head),
        egui::pos2(start + length - head, y + half_shaft),
        egui::pos2(start, y + half_shaft),
    ];
    // Not convex: filled as two convex parts, outlined as one closed line.
    painter.add(egui::Shape::convex_polygon(
        vec![points[0], points[1], points[5], points[6]],
        fill,
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![points[2], points[3], points[4]],
        fill,
        egui::Stroke::NONE,
    ));
    painter.add(egui::Shape::closed_line(points, border));
}
