//! A status banner across the whole width of the view: white, mostly opaque, with black text
//! about one line high, for states in which the user waits for something (the simulation
//! starting or restarting) or must know something (a start failed).
//!
//! [`show`] draws it for one frame; not calling it hides it. It is drawn into the layer of the
//! 3D view, on top of the planet but below every window and button, so it never hides the
//! parameter panel or its ⚙ button. It is centred vertically, where the user looks first, and
//! covers only a strip of the planet.

use eframe::egui;

/// Opacity of the banner background (of 255).
const BACKGROUND_ALPHA: u8 = 230;
/// Padding above and below the text, as a fraction of the font size.
const VERTICAL_PADDING: f32 = 0.3;
/// Padding left and right of the text, as a fraction of the font size.
const HORIZONTAL_PADDING: f32 = 1.0;

/// The text of the banner for the state of the simulation, or `None` if it is running normally.
///
/// `restarted` tells whether the simulation was started by a restart rather than when the
/// view opened; `time` is the simulated time of its latest surface update (`None` until the
/// first arrives), `failure` why it failed, if it did.
pub fn simulation_text(
    restarted: bool,
    time: Option<f64>,
    failure: Option<&str>,
) -> Option<String> {
    match (failure, time) {
        (Some(failure), _) => Some(format!("Simulation failed: {failure}")),
        (None, Some(_)) => None,
        (None, None) if restarted => Some("Restarting the simulation…".to_owned()),
        (None, None) => Some("Starting the simulation…".to_owned()),
    }
}

/// Draws the banner with `text` across the view of the root `ui` for this frame. Text that
/// does not fit the width wraps, and the banner grows with it.
pub fn show(ui: &egui::Ui, text: &str) {
    let screen = ui.ctx().content_rect();
    let font = egui::TextStyle::Heading.resolve(ui.style());
    let horizontal_padding = HORIZONTAL_PADDING * font.size;
    let vertical_padding = VERTICAL_PADDING * font.size;
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        font,
        egui::Color32::BLACK,
        (screen.width() - 2.0 * horizontal_padding).max(1.0),
    );
    job.halign = egui::Align::Center;
    let galley = ui.painter().layout_job(job);

    let height = galley.size().y + 2.0 * vertical_padding;
    let banner = egui::Rect::from_center_size(screen.center(), egui::vec2(screen.width(), height));
    let painter = ui.painter().with_clip_rect(screen);
    painter.rect_filled(
        banner,
        0.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, BACKGROUND_ALPHA),
    );
    // A centred galley is laid out around x = 0.
    let origin = egui::pos2(screen.center().x, banner.top() + vertical_padding);
    painter.galley(origin, galley, egui::Color32::BLACK);
}

#[cfg(test)]
mod tests {
    use super::simulation_text;

    #[test]
    fn starting_until_the_first_update() {
        assert_eq!(
            simulation_text(false, None, None).as_deref(),
            Some("Starting the simulation…")
        );
        assert_eq!(simulation_text(false, Some(0.0), None), None);
        assert_eq!(simulation_text(false, Some(120.0), None), None);
    }

    #[test]
    fn restarting_until_the_first_update_of_the_new_run() {
        assert_eq!(
            simulation_text(true, None, None).as_deref(),
            Some("Restarting the simulation…")
        );
        assert_eq!(simulation_text(true, Some(0.0), None), None);
    }

    #[test]
    fn failures_stay_shown() {
        for (restarted, time) in [(false, None), (true, None), (false, Some(50.0))] {
            assert_eq!(
                simulation_text(restarted, time, Some("no memory")).as_deref(),
                Some("Simulation failed: no memory")
            );
        }
    }
}
