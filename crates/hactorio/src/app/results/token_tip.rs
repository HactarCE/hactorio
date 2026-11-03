//! The token tip: a tooltip about the prototype or variable under the pointer.

use std::collections::BTreeMap;

use egui::{RichText, Sense, TextStyle, Ui};
use hactorio_core::Settings;
use hactorio_core::new_eval::{
    Amount, Item, Output, Quality, Quantity, Resource, Value, format_facets,
    format_number, format_value, is_zero, line_hash,
};
use hactorio_data::{PrototypeKind, Registry};

use super::{ResultsContext, Token, glyph_at};
use crate::app::completion::definition_name;

/// Width of a token tip.
const TOKEN_TIP_WIDTH: f32 = 360.0;

impl ResultsContext<'_> {
    /// Whether `token` has anything to describe.
    ///
    /// A token that resolves to nothing (a keyword, a typo, a forward
    /// reference, a stale result) must not open a tip at all.
    pub(super) fn describes(&self, token: &Token) -> bool {
        match token {
            Token::Prototype { glyph, .. } => self.registry.entry_by_glyph(*glyph).is_some(),
            Token::Variable { line, name, .. } => self.variable_value(*line, name).is_some(),
        }
    }

    pub(crate) fn token_tip_ui(&self, ui: &mut Ui, token: &Token) {
        match token {
            Token::Prototype { glyph, .. } => self.prototype_tip_ui(ui, *glyph),
            Token::Variable { line, name, .. } => self.variable_tip_ui(ui, *line, name),
        }
    }

    /// The tip for `token`, shown next to it.
    pub(crate) fn show_token_tip(&self, ctx: &egui::Context, token: Token) {
        if !self.describes(&token) {
            return;
        }
        let anchor = token.rect();
        let pointer = ctx.pointer_hover_pos().unwrap_or(anchor.center());
        let layer = ctx.layer_id_at(pointer).unwrap_or_else(|| {
            egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("token_tip"))
        });
        egui::Tooltip::always_open(ctx.clone(), layer, token.tip_id(), anchor)
            .width(TOKEN_TIP_WIDTH)
            .show(|ui| self.token_tip_ui(ui, &token));
    }

    fn prototype_tip_ui(&self, ui: &mut Ui, glyph: char) {
        let Some(entry) = self.registry.entry_by_glyph(glyph) else {
            return;
        };
        let name = entry.display_name.as_deref().unwrap_or(&entry.id);
        ui.heading(format!("{} {name}", entry.glyph));

        self.rows_grid(ui, "common", vec![("id".to_string(), entry.id.to_string())]);

        if let Some(item) = &entry.item {
            self.section_grid(
                ui,
                "item",
                vec![("stack size".to_string(), item.stack_size.to_string())],
            );
        }

        if let Some(module) = &entry.module {
            let mut rows = vec![
                ("category".to_string(), module.category.to_string()),
                ("tier".to_string(), module.tier.to_string()),
                ("stack size".to_string(), module.stack_size.to_string()),
            ];
            for (label, value) in [
                ("speed", module.effect.speed),
                ("productivity", module.effect.productivity),
                ("consumption", module.effect.consumption),
                ("pollution", module.effect.pollution),
                ("quality", module.effect.quality),
            ] {
                if !is_zero(value) {
                    rows.push((
                        label.to_string(),
                        signed_percent(value, self.settings),
                    ));
                }
            }
            self.section_grid(ui, "module", rows);
        }

        if let Some(recipe) = entry.recipe.map(|id| self.registry.recipe(id)) {
            let mut rows = vec![
                (
                    "categories".to_string(),
                    recipe
                        .categories
                        .iter()
                        .map(AsRef::as_ref)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                (
                    "craft time".to_string(),
                    format_value(
                        &Value::quantity(Quantity::time(recipe.craft_time)),
                        self.settings,
                        self.registry,
                    ),
                ),
                (
                    "productivity allowed".to_string(),
                    if recipe.allow_productivity { "yes" } else { "no" }.to_string(),
                ),
            ];
            if !recipe.ingredients.is_empty() {
                rows.push((
                    "ingredients".to_string(),
                    recipe
                        .ingredients
                        .iter()
                        .map(|ingredient| {
                            self.amount_text(ingredient.kind, &ingredient.name, ingredient.amount)
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
            if !recipe.products.is_empty() {
                rows.push((
                    "products".to_string(),
                    recipe
                        .products
                        .iter()
                        .map(|product| self.amount_text(product.kind, &product.name, product.amount))
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
            self.section_grid(ui, "recipe", rows);
        }

        if let Some(machine) = entry.machine() {
            let mut rows = vec![
                (
                    "speed".to_string(),
                    format_number(machine.speed),
                ),
                ("module slots".to_string(), machine.module_slots.to_string()),
            ];
            if !is_zero(machine.base_productivity) {
                rows.push((
                    "built-in productivity".to_string(),
                    percent(machine.base_productivity),
                ));
            }
            rows.push((
                "energy usage".to_string(),
                format_value(&watts(machine.energy_usage_w), self.settings, self.registry),
            ));
            rows.push((
                "pollution".to_string(),
                format!("{}/min", format_number(machine.pollution_per_minute)),
            ));
            rows.push((
                "categories".to_string(),
                machine
                    .categories
                    .iter()
                    .map(AsRef::as_ref)
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
            let mut allowed = Vec::new();
            for (label, enabled) in [
                ("speed", machine.allowed_effects.speed),
                ("productivity", machine.allowed_effects.productivity),
                ("consumption", machine.allowed_effects.consumption),
                ("pollution", machine.allowed_effects.pollution),
                ("quality", machine.allowed_effects.quality),
            ] {
                if enabled {
                    allowed.push(label);
                }
            }
            rows.push(("allowed effects".to_string(), allowed.join(", ")));
            self.section_grid(ui, "machine", rows);
        }
    }

    fn variable_tip_ui(&self, ui: &mut Ui, line: usize, name: &str) {
        let Some(value) = self.variable_value(line, name) else {
            return;
        };
        let formatted = format_value(value, self.settings, self.registry);
        ui.heading(name);
        ui.label(RichText::new(format!("{name} = {formatted}")).strong());
        self.value_tip_ui(ui, value);
    }

    /// The value a `name` at `line` refers to, if that name is a defined
    /// variable whose result is still current.
    fn variable_value(&self, line: usize, name: &str) -> Option<&Value> {
        let definition_line = self.definition_line(line, name)?;
        let tab = self.tabs.get(self.active_tab)?;
        let text = tab.text.split('\n').nth(definition_line)?;
        if self.outputs.hashes.get(definition_line).copied() != Some(line_hash(text)) {
            return None;
        }
        match self.outputs.outputs.get(definition_line)? {
            Output::Value(value) => Some(value),
            _ => None,
        }
    }

    fn definition_line(&self, line: usize, name: &str) -> Option<usize> {
        let tab = self.tabs.get(self.active_tab)?;
        let lines: Vec<&str> = tab.text.split('\n').take(line).collect();
        lines
            .iter()
            .rposition(|text| definition_name(text) == Some(name))
    }

    /// The facets of a value, one labelled group.
    fn value_tip_ui(&self, ui: &mut Ui, value: &Value) {
        let lines = format_facets(value, self.settings, self.registry);
        if lines.is_empty() {
            return;
        }
        ui.add_space(6.0);
        for line in lines {
            // The process panel's section headers are titles, not values.
            if line == "in" || line == "out" {
                ui.label(RichText::new(line).strong());
            } else {
                ui.label(line);
            }
        }
    }

    fn section_grid(&self, ui: &mut Ui, title: &str, rows: Vec<(String, String)>) {
        ui.add_space(6.0);
        ui.label(RichText::new(title).strong());
        self.rows_grid(ui, title, rows);
    }

    fn rows_grid(&self, ui: &mut Ui, id: &str, rows: Vec<(String, String)>) {
        egui::Grid::new(ui.make_persistent_id(("token_tip", id)))
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for (label, value) in rows {
                    ui.label(label);
                    ui.label(value);
                    ui.end_row();
                }
            });
    }

    fn amount_text(&self, kind: PrototypeKind, name: &str, amount: f64) -> String {
        let Some(id) = self.registry.entry_id(name) else {
            return format!("{amount} {name}");
        };
        let resource = match kind {
            PrototypeKind::Item => Resource::Item(Item {
                id,
                quality: Quality::Normal,
            }),
            PrototypeKind::Fluid => Resource::Fluid(id),
        };
        let quantity = Quantity {
            amount: Amount::Resources(BTreeMap::from([(resource, amount)])),
            per: None,
            second: 0,
            joule: 0,
            craft: 0,
        };
        format_value(
            &Value::quantity(quantity),
            self.settings,
            self.registry,
        )
    }
}

/// One watt, as a quantity.
fn watts(value: f64) -> Value {
    let quantity = Quantity {
        joule: 1,
        second: -1,
        ..Quantity::number(value)
    };
    Value::quantity(quantity)
}

fn percent(value: f64) -> String {
    format!("{}%", format_number(value * 100.0))
}

fn signed_percent(value: f64, _settings: &Settings) -> String {
    let text = percent(value);
    if value > 0.0 {
        format!("+{text}")
    } else {
        text
    }
}

/// Draws a monospace value line and returns the prototype token under the pointer.
pub(super) fn value_line(ui: &mut Ui, text: &str, registry: &Registry) -> Option<Token> {
    let font_id = TextStyle::Monospace.resolve(ui.style());
    let color = ui.visuals().text_color();
    let galley = ui
        .painter()
        .layout(text.to_string(), font_id, color, ui.available_width());
    let (rect, _response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley.clone(), color);

    let pointer = ui.ctx().pointer_hover_pos()?;
    glyph_at(registry, &galley, rect.min, pointer)
        .map(|(_, glyph, rect)| Token::Prototype { glyph, rect })
}
