//! The multiline result panel: every facet of the caret line's result, floating
//! beside the results column.

use std::sync::Arc;

use egui::{Color32, RichText, Ui};
use hactorio_core::Settings;
use hactorio_core::new_eval::{Output, format_facets, line_hash};
use hactorio_data::Registry;

use super::token_tip::value_line;
use super::{ResultsContext, Token};

/// Frame chrome between the panel's outer corner and its content.
pub(crate) fn multiline_result_inset(style: &egui::Style) -> egui::Vec2 {
    let frame = egui::Frame::popup(style);
    frame.inner_margin.left_top() + egui::vec2(frame.stroke.width, frame.stroke.width)
}

/// Where to place the panel so its content starts at `anchor`.
pub(crate) fn multiline_result_pos(
    style: &egui::Style,
    anchor: egui::Pos2,
    screen: egui::Rect,
    width: f32,
) -> egui::Pos2 {
    let inset = multiline_result_inset(style);
    egui::pos2(
        (anchor.x - inset.x)
            .min(screen.right() - width)
            .max(screen.left() + 8.0),
        (anchor.y - inset.y)
            .min(screen.bottom() - 160.0)
            .max(screen.top() + 8.0),
    )
}

impl ResultsContext<'_> {
    /// Shows the full result for `line`, floating beside the results column.
    pub(crate) fn show_multiline_result(
        &self,
        ctx: &egui::Context,
        anchor: egui::Pos2,
        line: usize,
        width: f32,
    ) {
        let registry = self.registry;
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return;
        };
        let Some(text) = tab.text.split('\n').nth(line) else {
            return;
        };
        let Some(output) = self.outputs.outputs.get(line) else {
            return;
        };
        if self.outputs.hashes.get(line).copied() != Some(line_hash(text)) {
            return;
        }

        let screen = ctx.viewport_rect();
        let min_width = width.clamp(240.0, 420.0);
        let style = ctx.style_of(ctx.theme());
        let mut measure_style = (*style).clone();
        measure_style.wrap_mode = Some(egui::TextWrapMode::Extend);
        let mut measure = Ui::new(
            ctx.clone(),
            egui::Id::unique(("multiline_result_measure", line)),
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(egui::Pos2::ZERO, screen.size()))
                .layout(egui::Layout::top_down(egui::Align::Min))
                .style(Arc::new(measure_style))
                .invisible()
                .sizing_pass(),
        );
        multiline_result_ui(&mut measure, output, self.settings, registry);
        let content_width = measure.min_rect().width().max(min_width);
        let total_width = content_width + 2.0 * multiline_result_inset(&style).x;
        let pos = multiline_result_pos(&style, anchor, screen, total_width);
        let token = egui::Area::new(egui::Id::unique("multiline_result"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .interactable(false)
            .show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        ui.set_width(content_width);
                        multiline_result_ui(ui, output, self.settings, registry)
                    })
                    .inner
            })
            .inner;
        if let Some(token) = token {
            self.show_token_tip(ctx, token);
        }
    }
}

fn multiline_result_ui(
    ui: &mut Ui,
    output: &Output,
    settings: &Settings,
    registry: &Registry,
) -> Option<Token> {
    match output {
        Output::Blank | Output::None => None,
        Output::Error { message, .. } => {
            let color = Color32::from_rgb(220, 90, 90);
            ui.label(RichText::new("error").monospace().color(color));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(RichText::new(message).color(color));
            None
        }
        Output::Value(value) => {
            let mut token = None;
            for line in format_facets(value, settings, registry) {
                // The process panel's section headers are titles, not values.
                if line == "in" || line == "out" {
                    ui.label(RichText::new(line).weak());
                } else {
                    token = token.or(value_line(ui, &line, registry));
                }
            }
            token
        }
    }
}
