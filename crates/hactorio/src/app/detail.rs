//! Hover details: icon hit-testing, the full results popup, and the breakdown.

use std::sync::Arc;

use egui::{Color32, RichText, Sense, TextStyle, Ui};
use hactorio_core::{
    BaseUnit, LineOutput, Output, RecipeQuantity, Settings, StatementOutput, Value, breakdown,
    format_amount, format_energy, format_output, format_power, format_value, line_hash,
};
use hactorio_data::Registry;

use crate::document::Tab;

/// Frame chrome between the detail popup's outer corner and its content.
pub(super) fn detail_popup_inset(style: &egui::Style) -> egui::Vec2 {
    let frame = egui::Frame::popup(style);
    frame.inner_margin.left_top() + egui::vec2(frame.stroke.width, frame.stroke.width)
}

/// Where to place the detail popup so its content starts at `anchor`.
pub(super) fn detail_popup_pos(
    style: &egui::Style,
    anchor: egui::Pos2,
    screen: egui::Rect,
    width: f32,
) -> egui::Pos2 {
    let inset = detail_popup_inset(style);
    egui::pos2(
        (anchor.x - inset.x)
            .min(screen.right() - width)
            .max(screen.left() + 8.0),
        (anchor.y - inset.y)
            .min(screen.bottom() - 160.0)
            .max(screen.top() + 8.0),
    )
}

/// The icon glyph under `pointer` in a laid-out text, with its row and rect.
pub(super) fn hovered_icon(
    registry: &Registry,
    galley: &egui::Galley,
    origin: egui::Pos2,
    pointer: egui::Pos2,
) -> Option<(usize, egui::Rect)> {
    for (row_index, row) in galley.rows.iter().enumerate() {
        let top = origin.y + row.pos.y;
        if pointer.y < top || pointer.y > top + row.size.y {
            continue;
        }
        for glyph in &row.glyphs {
            if registry.entry_by_glyph(glyph.chr).is_none() {
                continue;
            }
            let rect = egui::Rect::from_min_size(
                egui::pos2(origin.x + row.pos.x + glyph.pos.x, top),
                egui::vec2(glyph.advance_width, row.size.y),
            );
            if rect.contains(pointer) {
                return Some((row_index, rect));
            }
        }
    }
    None
}

/// The parts of the app the detail and breakdown popups read.
pub(super) struct DetailContext<'a> {
    pub tabs: &'a [Tab],
    pub active: usize,
    pub outputs: &'a [StatementOutput],
    pub settings: &'a Settings,
    pub registry: Option<&'a Registry>,
}

impl DetailContext<'_> {
    fn breakdown_ui(&self, ui: &mut Ui, line: usize) {
        let (Some(registry), Some(tab)) = (self.registry, self.tabs.get(self.active)) else {
            return;
        };
        let (Some(text), Some(output)) = (tab.text.split('\n').nth(line), self.outputs.get(line))
        else {
            return;
        };
        if output.source_hash != line_hash(text) {
            return;
        }
        let Some(value) = &output.eval else {
            return;
        };

        ui.heading("Breakdown");
        let detail = breakdown(value, self.settings, registry);
        for section in detail.sections {
            ui.add_space(6.0);
            let title = section.title.clone();
            ui.label(RichText::new(&title).strong());
            egui::Grid::new(("breakdown", &title))
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for row in section.rows {
                        ui.label(row.label);
                        ui.label(row.value);
                        ui.end_row();
                    }
                });
        }
    }

    /// The breakdown for `line`, shown next to a hovered icon.
    pub(super) fn breakdown_popup(&self, ctx: &egui::Context, line: usize, anchor: egui::Rect) {
        let (Some(tab), Some(output)) = (self.tabs.get(self.active), self.outputs.get(line)) else {
            return;
        };
        let Some(text) = tab.text.split('\n').nth(line) else {
            return;
        };
        if output.source_hash != line_hash(text) || output.eval.is_none() {
            return;
        }

        let pointer = ctx.pointer_hover_pos().unwrap_or(anchor.center());
        let layer = ctx.layer_id_at(pointer).unwrap_or_else(|| {
            egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("breakdown"))
        });
        egui::Tooltip::always_open(
            ctx.clone(),
            layer,
            egui::Id::unique(("breakdown", line)),
            anchor,
        )
        .width(360.0)
        .show(|ui| self.breakdown_ui(ui, line));
    }

    /// Full results for the line under the caret, floating beside it.
    pub(super) fn detail_panel(
        &self,
        ctx: &egui::Context,
        anchor: egui::Pos2,
        line: usize,
        width: f32,
    ) {
        let (Some(registry), Some(tab)) = (self.registry, self.tabs.get(self.active)) else {
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
            egui::Id::unique(("line_detail_measure", line)),
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(egui::Pos2::ZERO, screen.size()))
                .layout(egui::Layout::top_down(egui::Align::Min))
                .style(Arc::new(measure_style))
                .invisible()
                .sizing_pass(),
        );
        detail_ui(&mut measure, statement, self.settings, registry);
        let content_width = measure.min_rect().width().max(min_width);
        let total_width = content_width + 2.0 * detail_popup_inset(&style).x;
        let pos = detail_popup_pos(&style, anchor, screen, total_width);
        let hovered = egui::Area::new(egui::Id::unique("line_detail"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .interactable(false)
            .show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        ui.set_width(content_width);
                        detail_ui(ui, statement, self.settings, registry)
                    })
                    .inner
            })
            .inner;
        if let Some(anchor) = hovered {
            self.breakdown_popup(ctx, line, anchor);
        }
    }
}

/// Draws a monospace value and returns the rect of any hovered icon glyph in it.
fn icon_label(ui: &mut Ui, text: &str, registry: &Registry) -> Option<egui::Rect> {
    let font_id = TextStyle::Monospace.resolve(ui.style());
    let color = ui.visuals().text_color();
    let galley = ui
        .painter()
        .layout(text.to_string(), font_id, color, ui.available_width());
    let (rect, _response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley.clone(), color);

    let pointer = ui.ctx().pointer_hover_pos()?;
    hovered_icon(registry, &galley, rect.min, pointer).map(|(_, rect)| rect)
}

fn detail_ui(
    ui: &mut Ui,
    statement: &StatementOutput,
    settings: &Settings,
    registry: &Registry,
) -> Option<egui::Rect> {
    let mut hovered = None;
    match &statement.output {
        LineOutput::Nothing => {}
        LineOutput::Error(error) => {
            ui.label(RichText::new(&error.message).color(Color32::from_rgb(220, 90, 90)));
        }
        LineOutput::Value(value) => {
            let formatted = format_output(value, settings, registry);
            hovered = icon_label(ui, &formatted.headline, registry);

            let Output::Recipe(recipe) = value else {
                return hovered;
            };
            let rate = matches!(recipe.quantity, RecipeQuantity::Machines(_));

            let mut units: Vec<(BaseUnit, f64)> = recipe
                .net
                .iter()
                .filter(|(unit, _)| **unit != BaseUnit::Craft)
                .map(|(unit, amount)| (*unit, *amount))
                .collect();
            units.sort_by_key(|(unit, _)| unit_label(registry, *unit));

            let mut produces = Vec::new();
            let mut consumes = Vec::new();
            for (unit, amount) in units {
                let text = format_amount(amount.abs(), unit, rate, settings, registry);
                if amount > 0.0 {
                    produces.push(text);
                } else {
                    consumes.push(text);
                }
            }

            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                ui.vertical(|ui| {
                    ui.set_min_width(90.0);
                    ui.label(RichText::new("produces").weak());
                    for produce in &produces {
                        hovered = hovered.or(icon_label(ui, produce, registry));
                    }
                });
                ui.vertical(|ui| {
                    ui.set_min_width(90.0);
                    ui.label(RichText::new("consumes").weak());
                    for consume in &consumes {
                        hovered = hovered.or(icon_label(ui, consume, registry));
                    }
                });
            });

            if let Some(craft) = recipe.net.get(&BaseUnit::Craft).copied() {
                let value = if rate {
                    Value::new(craft, [(BaseUnit::Craft, 1), (BaseUnit::Second, -1)])
                } else {
                    Value::crafts(craft)
                };
                hovered = hovered.or(icon_label(
                    ui,
                    &format_value(&value, settings, registry),
                    registry,
                ));
            }

            let glyph = registry.entry_by_index(recipe.machine).glyph;
            hovered = hovered.or(icon_label(
                ui,
                &format!(
                    "{}/{glyph}",
                    format_power(recipe.per_machine_power_w, settings)
                ),
                registry,
            ));
            let count = recipe.quantity.magnitude();
            if rate && count != 1.0 {
                hovered = hovered.or(icon_label(
                    ui,
                    &format!(
                        "{} total",
                        format_power(recipe.per_machine_power_w * count, settings)
                    ),
                    registry,
                ));
            }
            if !rate && let Some(energy) = recipe.total_energy_j {
                hovered = hovered.or(icon_label(
                    ui,
                    &format!("{} total energy", format_energy(energy, settings)),
                    registry,
                ));
            }
        }
    }
    hovered
}

fn unit_label(registry: &Registry, unit: BaseUnit) -> String {
    match unit {
        BaseUnit::Item(id) | BaseUnit::Fluid(id) => registry.entry_by_index(id).id.to_string(),
        BaseUnit::Craft => "craft".to_string(),
        BaseUnit::Second => "second".to_string(),
        BaseUnit::Joule => "joule".to_string(),
    }
}

pub(super) fn output_text(
    statement: &StatementOutput,
    settings: &Settings,
    registry: &Registry,
) -> Option<String> {
    match &statement.output {
        LineOutput::Nothing => None,
        LineOutput::Error(error) => Some(error.message.clone()),
        LineOutput::Value(value) => {
            let formatted = format_output(value, settings, registry);
            let mut text = formatted.headline;
            for row in formatted.rows {
                text.push_str(" · ");
                text.push_str(&row);
            }
            Some(text)
        }
    }
}
