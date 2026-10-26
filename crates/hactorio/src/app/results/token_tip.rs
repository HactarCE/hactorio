//! The token tip: a tooltip about the prototype or variable under the pointer.

use egui::{RichText, Sense, TextStyle, Ui};
use hactorio_core::facet_eval::{LineOutput, StatementOutput, line_hash};
use hactorio_core::facet_format::{
    format_capacity, format_config, format_facet, format_machine, format_machine_stats,
    format_process_craft, format_process_energy, format_process_inputs,
    format_process_machine_lines, format_process_outputs, format_rate,
};
use hactorio_core::facets::FacetValue;
use hactorio_core::{BaseUnit, Settings, Value, format_power, format_value};
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

        // The untitled section holds properties common to every kind.
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
            let effects = [
                ("speed", module.effect.speed),
                ("productivity", module.effect.productivity),
                ("consumption", module.effect.consumption),
                ("pollution", module.effect.pollution),
                ("quality", module.effect.quality),
            ];
            for (label, value) in effects {
                if !hactorio_core::is_zero(value) {
                    rows.push((
                        label.to_string(),
                        signed_percent(value, self.settings, self.registry),
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
                        &Value::seconds(recipe.craft_time),
                        self.settings,
                        self.registry,
                    ),
                ),
                (
                    "productivity allowed".to_string(),
                    if recipe.allow_productivity {
                        "yes"
                    } else {
                        "no"
                    }
                    .to_string(),
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
                        .map(|product| {
                            self.amount_text(product.kind, &product.name, product.amount)
                        })
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
                    format_value(&Value::scalar(machine.speed), self.settings, self.registry),
                ),
                ("module slots".to_string(), machine.module_slots.to_string()),
            ];
            if !hactorio_core::is_zero(machine.base_productivity) {
                rows.push((
                    "built-in productivity".to_string(),
                    percent(machine.base_productivity, self.settings, self.registry),
                ));
            }
            rows.push((
                "energy usage".to_string(),
                format_power(machine.energy_usage_w, self.settings),
            ));
            rows.push((
                "pollution".to_string(),
                format!(
                    "{}/min",
                    format_value(
                        &Value::scalar(machine.pollution_per_minute),
                        self.settings,
                        self.registry
                    )
                ),
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
            if machine.allowed_effects.speed {
                allowed.push("speed");
            }
            if machine.allowed_effects.productivity {
                allowed.push("productivity");
            }
            if machine.allowed_effects.consumption {
                allowed.push("consumption");
            }
            if machine.allowed_effects.pollution {
                allowed.push("pollution");
            }
            if machine.allowed_effects.quality {
                allowed.push("quality");
            }
            rows.push(("allowed effects".to_string(), allowed.join(", ")));
            self.section_grid(ui, "machine", rows);
        }
    }

    fn variable_tip_ui(&self, ui: &mut Ui, line: usize, name: &str) {
        let Some((statement, value)) = self.variable_value(line, name) else {
            return;
        };
        let formatted = format_facet(value, self.settings, self.registry);

        ui.heading(name);
        ui.label(RichText::new(format!("{name} = {formatted}")).strong());
        if let Some(value) = &statement.value {
            self.value_tip_ui(ui, value);
        }
    }

    /// The value a `name` at `line` refers to, if that name is a defined variable
    /// whose result is still current.
    fn variable_value(&self, line: usize, name: &str) -> Option<(&StatementOutput, &FacetValue)> {
        let definition_line = self.definition_line(line, name)?;
        let tab = self.tabs.get(self.active_tab)?;
        let text = tab.text.split('\n').nth(definition_line)?;
        let statement = self.outputs.get(definition_line)?;
        if statement.source_hash != line_hash(text) {
            return None;
        }
        let LineOutput::Value(value) = &statement.output else {
            return None;
        };
        Some((statement, value))
    }

    fn definition_line(&self, line: usize, name: &str) -> Option<usize> {
        let tab = self.tabs.get(self.active_tab)?;
        let lines: Vec<&str> = tab.text.split('\n').take(line).collect();
        lines
            .iter()
            .rposition(|text| definition_name(text) == Some(name))
    }

    fn value_tip_ui(&self, ui: &mut Ui, value: &FacetValue) {
        let mut rows = Vec::new();
        if let Some(item) = &value.item {
            let value = Value::new(
                item.coef(),
                item.units()
                    .iter()
                    .map(|(unit, exponent)| (*unit, *exponent)),
            );
            rows.push((
                "item".to_string(),
                format_value(&value, self.settings, self.registry),
            ));
        }
        if let Some(rate) = &value.rate {
            rows.push((
                "rate".to_string(),
                format_rate(rate, self.settings, self.registry),
            ));
        }
        if let Some(capacity) = &value.capacity {
            rows.push((
                "capacity".to_string(),
                format_capacity(capacity, self.settings, self.registry),
            ));
        }
        if let Some(machine) = &value.machine {
            rows.push((
                "machine".to_string(),
                format_machine(machine, self.settings, self.registry),
            ));
            match format_machine_stats(machine, self.settings, self.registry) {
                Ok(stats) => rows.extend(stats),
                Err(error) => rows.push(("error".to_string(), error)),
            }
        }
        if !rows.is_empty() {
            self.section_grid(ui, "value", rows);
        }
        if let Some(process) = &value.process {
            self.process_tip_ui(ui, process);
        }
        let mut rows = Vec::new();
        if let Some(config) = &value.config {
            rows.push((
                "config".to_string(),
                format_config(config, self.settings, self.registry),
            ));
        }
        if let Some(recipe) = value.recipe {
            rows.push((
                "recipe".to_string(),
                self.registry.recipe(recipe).id.to_string(),
            ));
        }
        if !rows.is_empty() {
            ui.add_space(6.0);
            self.rows_grid(ui, "value-tail", rows);
        }
    }

    fn process_tip_ui(&self, ui: &mut Ui, process: &hactorio_core::facets::ProcessFacet) {
        let mut header = format_process_machine_lines(process, self.settings, self.registry);
        header.extend(format_process_craft(process, self.settings));
        self.process_lines(ui, header);
        let mut inputs = format_process_inputs(process, self.settings, self.registry);
        inputs.push(format_process_energy(process, self.settings));
        self.process_section(ui, "inputs", inputs);
        self.process_section(
            ui,
            "outputs",
            format_process_outputs(process, self.settings, self.registry),
        );
    }

    fn process_lines(&self, ui: &mut Ui, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        ui.add_space(6.0);
        for line in lines {
            ui.label(line);
        }
    }

    fn process_section(&self, ui: &mut Ui, title: &str, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        ui.add_space(6.0);
        ui.label(RichText::new(title).strong());
        for line in lines {
            ui.label(line);
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
        let unit = match kind {
            PrototypeKind::Item => BaseUnit::Item(id),
            PrototypeKind::Fluid => BaseUnit::Fluid(id),
        };
        format_value(
            &Value::new(amount, [(unit, 1)]),
            self.settings,
            self.registry,
        )
    }
}

fn percent(value: f64, settings: &Settings, registry: &Registry) -> String {
    format!(
        "{}%",
        format_value(&Value::scalar(value * 100.0), settings, registry)
    )
}

fn signed_percent(value: f64, settings: &Settings, registry: &Registry) -> String {
    let text = percent(value, settings, registry);
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
