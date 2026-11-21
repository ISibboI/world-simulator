//! The mode switcher in the top left corner of the 3D view: a drop-down list of the render
//! modes and the legend of the active one (colour bar, swatches, arrow scale), which can be
//! hidden. It is free there: the parameter panel is in the top right corner, the cell widget
//! in the bottom left, and the banner across the middle. Like those, the legend starts hidden
//! on narrow screens (phones), where only the drop-down list remains.

use eframe::egui;

use super::{Legend, RenderModeId};

/// Screens narrower than this (in points) start with the legend hidden, as the other panels.
const NARROW_SCREEN: f32 = 600.0;
/// Largest width of the legend in points.
const MAX_WIDTH: f32 = 260.0;
/// Distance from the corner of the window, in points.
const MARGIN: f32 = 8.0;
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
}

/// The switcher's state.
#[derive(Default)]
pub struct ModeSwitcher {
    /// Whether the legend is shown; decided by the screen width on the first frame.
    legend_open: Option<bool>,
}

impl ModeSwitcher {
    /// Shows the switcher; changes `mode` if the user chose another one. Returns whether that
    /// happened and the area the switcher covers (the parameter panel's button keeps clear of
    /// it on narrow screens).
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
        let response = egui::Area::new(egui::Id::unique("render mode switcher"))
            .anchor(egui::Align2::LEFT_TOP, [MARGIN, MARGIN])
            .show(ctx, |ui| {
                if *legend_open {
                    egui::Frame::window(ui.style()).show(ui, |ui| {
                        ui.set_max_width(width);
                        controls(ui, mode, legend_open);
                        ui.separator();
                        legend(ui, view.legend, view, width);
                    });
                } else {
                    // Hidden, only the controls remain, like the buttons of the other panels.
                    controls(ui, mode, legend_open);
                }
            });
        (changed, response.response.rect)
    }
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
    if !legend.swatches.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for swatch in &legend.swatches {
                let [r, g, b] = swatch.color;
                let (rect, _) = ui
                    .allocate_exact_size(egui::vec2(bar_height, bar_height), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, 2.0, egui::Color32::from_rgb(r, g, b));
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    ui.visuals().widgets.noninteractive.bg_stroke,
                    egui::StrokeKind::Outside,
                );
                ui.label(swatch.label.as_ref());
            }
        });
    }
    if let Some(arrows) = &legend.arrows {
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
