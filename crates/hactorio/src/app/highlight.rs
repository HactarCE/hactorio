//! Syntax highlighting: the editor's layout job, colored by token category.

use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat, Visuals};
use hactorio_core::{Highlight, highlights};

/// Layouts `text` with one section per plain run and per highlighted range.
///
/// Plain runs use the theme's text color; highlighted runs use [`color`].
pub(super) fn layout_job(text: &str, font_id: &FontId, visuals: &Visuals) -> LayoutJob {
    let mut job = LayoutJob::default();
    let plain = text_format(font_id, visuals.text_color());
    let mut cursor = 0;
    for (span, highlight) in highlights(text) {
        let start = span.start as usize;
        let end = span.end as usize;
        if cursor < start {
            job.append(&text[cursor..start], 0.0, plain.clone());
        }
        job.append(
            &text[start..end],
            0.0,
            text_format(font_id, color(highlight, visuals.dark_mode)),
        );
        cursor = end;
    }
    if job.sections.is_empty() || cursor < text.len() {
        job.append(&text[cursor..], 0.0, plain);
    }
    job.wrap.max_width = f32::INFINITY;
    job
}

fn text_format(font_id: &FontId, color: Color32) -> TextFormat {
    TextFormat {
        font_id: font_id.clone(),
        color,
        ..Default::default()
    }
}

/// The hard-coded color of `highlight` on a dark or light background.
fn color(highlight: Highlight, dark: bool) -> Color32 {
    match (highlight, dark) {
        (Highlight::OperatorKeyword, true) => Color32::from_rgb(198, 146, 234),
        (Highlight::OperatorKeyword, false) => Color32::from_rgb(136, 20, 160),
        (Highlight::UnitKeyword, true) => Color32::from_rgb(226, 192, 110),
        (Highlight::UnitKeyword, false) => Color32::from_rgb(150, 90, 0),
        (Highlight::Number, true) => Color32::from_rgb(160, 214, 150),
        (Highlight::Number, false) => Color32::from_rgb(0, 120, 70),
        (Highlight::Comment, true) => Color32::from_rgb(128, 138, 128),
        (Highlight::Comment, false) => Color32::from_rgb(110, 120, 110),
        (Highlight::Icon, true) => Color32::from_rgb(120, 186, 240),
        (Highlight::Icon, false) => Color32::from_rgb(20, 86, 170),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color_of(job: &LayoutJob, token: &str) -> Option<Color32> {
        job.sections.iter().find_map(|section| {
            let range = section.byte_range.clone();
            (token == &job.text[range.start.0..range.end.0]).then_some(section.format.color)
        })
    }

    fn sections_cover_text(job: &LayoutJob) -> bool {
        let mut end = 0;
        for section in &job.sections {
            if section.byte_range.start.0 != end {
                return false;
            }
            end = section.byte_range.end.0;
        }
        end == job.text.len()
    }

    #[test]
    fn sections_cover_the_whole_text() {
        let font_id = FontId::monospace(13.0);
        for text in ["", "plain text", "1 in :x: 2s # c"] {
            let job = layout_job(text, &font_id, &Visuals::dark());
            assert_eq!(job.text, text);
            assert!(sections_cover_text(&job), "{text:?}: {job:?}");
        }
    }

    #[test]
    fn each_category_has_its_own_color() {
        let font_id = FontId::monospace(13.0);
        let job = layout_job("1 in :x: 2s # c", &font_id, &Visuals::dark());
        assert_eq!(color_of(&job, "1"), Some(color(Highlight::Number, true)));
        assert_eq!(
            color_of(&job, "in"),
            Some(color(Highlight::OperatorKeyword, true))
        );
        assert_eq!(color_of(&job, ":x:"), Some(color(Highlight::Icon, true)));
        assert_eq!(color_of(&job, "2"), Some(color(Highlight::Number, true)));
        assert_eq!(
            color_of(&job, "s"),
            Some(color(Highlight::UnitKeyword, true))
        );
        assert_eq!(color_of(&job, "# c"), Some(color(Highlight::Comment, true)));
        assert_eq!(color_of(&job, " "), Some(Visuals::dark().text_color()));
    }

    #[test]
    fn light_and_dark_use_different_colors() {
        let font_id = FontId::monospace(13.0);
        let text = "1 in :x: 2s # c";
        let dark = layout_job(text, &font_id, &Visuals::dark());
        let light = layout_job(text, &font_id, &Visuals::light());
        for (token, highlight) in [
            ("1", Highlight::Number),
            ("in", Highlight::OperatorKeyword),
            (":x:", Highlight::Icon),
            ("s", Highlight::UnitKeyword),
            ("# c", Highlight::Comment),
        ] {
            assert_eq!(color_of(&light, token), Some(color(highlight, false)));
            assert_ne!(color_of(&dark, token), color_of(&light, token));
        }
    }
}
