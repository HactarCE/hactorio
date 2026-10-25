//! Syntax highlighting: the editor's layout job, colored by token category.

use egui::text::LayoutJob;
use egui::{FontId, TextFormat};
use hactorio_core::highlights;

use super::theme::Palette;

/// Layouts `text` with one section per plain run and per highlighted range.
///
/// Plain runs use the palette's text color; highlighted runs use
/// [`Palette::color`].
pub(super) fn layout_job(text: &str, font_id: &FontId, palette: Palette) -> LayoutJob {
    let mut job = LayoutJob::default();
    let plain = text_format(font_id, palette.plain);
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
            text_format(font_id, palette.color(highlight)),
        );
        cursor = end;
    }
    if job.sections.is_empty() || cursor < text.len() {
        job.append(&text[cursor..], 0.0, plain);
    }
    job.wrap.max_width = f32::INFINITY;
    job
}

fn text_format(font_id: &FontId, color: egui::Color32) -> TextFormat {
    TextFormat {
        font_id: font_id.clone(),
        color,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use egui::{Color32, Visuals};
    use hactorio_core::Highlight;

    use super::*;
    use crate::app::theme::palette;

    fn color_at(job: &LayoutJob, byte: usize) -> Option<Color32> {
        job.sections.iter().find_map(|section| {
            let range = &section.byte_range;
            (range.start.0 <= byte && byte < range.end.0).then_some(section.format.color)
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
        for text in ["", "plain text", "v = 1 in :x: 2s # c"] {
            let job = layout_job(text, &font_id, palette(&Visuals::dark()));
            assert_eq!(job.text, text);
            assert!(sections_cover_text(&job), "{text:?}: {job:?}");
        }
    }

    #[test]
    fn each_category_uses_its_palette_color() {
        let font_id = FontId::monospace(13.0);
        let palette = palette(&Visuals::dark());
        let text = "v = 1 in :x: 2s # c";
        let job = layout_job(text, &font_id, palette);
        for (span, highlight) in super::highlights(text) {
            let start = span.start as usize;
            let end = span.end as usize;
            // Adjacent spans of the same color share one section.
            assert!(
                job.sections.iter().any(|section| {
                    section.byte_range.start.0 <= start
                        && end <= section.byte_range.end.0
                        && section.format.color == palette.color(highlight)
                }),
                "{:?} at {start}..{end}",
                highlight
            );
        }
        assert_eq!(color_at(&job, 1), Some(palette.plain));
    }

    #[test]
    fn light_and_dark_use_different_colors() {
        let font_id = FontId::monospace(13.0);
        let text = "v = 1 in :x: 2s # c";
        let dark = palette(&Visuals::dark());
        let light = palette(&Visuals::light());
        let dark_job = layout_job(text, &font_id, dark);
        let light_job = layout_job(text, &font_id, light);
        for (byte, highlight) in [
            (0, Highlight::Variable),
            (4, Highlight::Number),
            (6, Highlight::OperatorKeyword),
            (9, Highlight::Icon),
            (14, Highlight::UnitKeyword),
            (16, Highlight::Comment),
        ] {
            assert_eq!(
                color_at(&light_job, byte),
                Some(light.color(highlight)),
                "{highlight:?}"
            );
            assert_ne!(
                color_at(&dark_job, byte),
                color_at(&light_job, byte),
                "{highlight:?}"
            );
        }
    }
}
