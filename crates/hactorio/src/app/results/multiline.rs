//! The multiline result panel: every facet of the caret line's result, floating
//! beside the results column.

use std::sync::Arc;

use egui::{Color32, RichText, Ui};
use hactorio_core::Settings;
use hactorio_core::facet_eval::{LineOutput, StatementOutput, line_hash};
use hactorio_core::facet_format::{
    format_facet, format_machine_stats, format_process_craft, format_process_energy,
    format_process_inputs, format_process_machine_lines, format_process_outputs,
};
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
        let Some(statement) = self.outputs.get(line) else {
            return;
        };
        if statement.source_hash != line_hash(text) {
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
        multiline_result_ui(&mut measure, statement, self.settings, registry);
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
                        multiline_result_ui(ui, statement, self.settings, registry)
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
    statement: &StatementOutput,
    settings: &Settings,
    registry: &Registry,
) -> Option<Token> {
    let mut token = None;
    match &statement.output {
        LineOutput::Nothing => {}
        LineOutput::Error(error) => {
            let color = Color32::from_rgb(220, 90, 90);
            ui.label(RichText::new("error").monospace().color(color));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(RichText::new(&error.message).color(color));
        }
        LineOutput::Value(value) => {
            if value.process.is_none() {
                let formatted = format_facet(value, settings, registry);
                token = token.or(value_line(ui, &formatted, registry));
            }
            if let Some(machine) = &value.machine {
                let stats = format_machine_stats(machine, settings, registry)
                    .unwrap_or_else(|error| vec![("error".to_string(), error)]);
                for (label, value) in stats {
                    let line = format!("{label}: {value}");
                    token = token.or(value_line(ui, &line, registry));
                }
            }

            if let Some(process) = &value.process {
                let mut header = format_process_machine_lines(process, settings, registry);
                header.push(format_process_craft(process, settings));
                for line in header {
                    token = token.or(value_line(ui, &line, registry));
                }
                let mut inputs = format_process_inputs(process, settings, registry);
                if let Some(energy) = format_process_energy(process, settings) {
                    inputs.push(energy);
                }
                for (title, lines) in [
                    ("inputs", inputs),
                    (
                        "outputs",
                        format_process_outputs(process, settings, registry),
                    ),
                ] {
                    if lines.is_empty() {
                        continue;
                    }
                    ui.label(RichText::new(title).weak());
                    for line in lines {
                        token = token.or(value_line(ui, &line, registry));
                    }
                }
            }
        }
    }
    token
}
