//! Hover details: icon and variable hit-testing, the full results popup, and
//! the details tooltip.
//!
//! The tooltip describes the hovered thing itself: a prototype's entry, or a
//! variable's value. It does not describe the whole line.

use std::sync::Arc;

use egui::{Color32, RichText, Sense, TextStyle, Ui};
use hactorio_core::facet_eval::{LineOutput, StatementOutput, line_hash};
use hactorio_core::facet_format::{
    format_capacity, format_config, format_facet, format_machine, format_rate,
};
use hactorio_core::facets::{Extent, FacetValue};
use hactorio_core::{
    BaseUnit, Settings, Value, format_amount, format_energy, format_power, format_value,
};
use hactorio_data::{Entry, PrototypeKind, Registry};

use super::completion::definition_name;
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
) -> Option<(usize, char, egui::Rect)> {
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
                return Some((row_index, glyph.chr, rect));
            }
        }
    }
    None
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// The identifier under `pointer` in a laid-out text, with its row and rect.
pub(super) fn hovered_identifier(
    galley: &egui::Galley,
    origin: egui::Pos2,
    pointer: egui::Pos2,
) -> Option<(usize, String, egui::Rect)> {
    for (row_index, row) in galley.rows.iter().enumerate() {
        let top = origin.y + row.pos.y;
        if pointer.y < top || pointer.y > top + row.size.y {
            continue;
        }
        for (index, glyph) in row.glyphs.iter().enumerate() {
            let left = origin.x + row.pos.x + glyph.pos.x;
            if pointer.x < left || pointer.x > left + glyph.advance_width {
                continue;
            }
            if !is_identifier_char(glyph.chr) {
                return None;
            }
            let mut first = index;
            while first > 0 && is_identifier_char(row.glyphs[first - 1].chr) {
                first -= 1;
            }
            let mut last = index;
            while last + 1 < row.glyphs.len() && is_identifier_char(row.glyphs[last + 1].chr) {
                last += 1;
            }
            let word: String = row.glyphs[first..=last]
                .iter()
                .map(|glyph| glyph.chr)
                .collect();
            if !word
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
            {
                return None;
            }
            let start = row.glyphs[first].pos.x;
            let end = row.glyphs[last].pos.x + row.glyphs[last].advance_width;
            let rect = egui::Rect::from_min_max(
                egui::pos2(origin.x + row.pos.x + start, top),
                egui::pos2(origin.x + row.pos.x + end, top + row.size.y),
            );
            return Some((row_index, word, rect));
        }
    }
    None
}

/// What the details tooltip describes.
#[derive(Clone)]
pub(super) enum HoverTarget {
    Entry {
        glyph: char,
        rect: egui::Rect,
    },
    Variable {
        line: usize,
        name: String,
        rect: egui::Rect,
    },
}

impl HoverTarget {
    pub(super) fn rect(&self) -> egui::Rect {
        match self {
            HoverTarget::Entry { rect, .. } | HoverTarget::Variable { rect, .. } => *rect,
        }
    }

    fn tooltip_id(&self) -> egui::Id {
        match self {
            HoverTarget::Entry { glyph, rect } => egui::Id::unique((
                "details",
                "entry",
                *glyph,
                rect.left_top().x.to_bits(),
                rect.left_top().y.to_bits(),
            )),
            HoverTarget::Variable { line, name, .. } => {
                egui::Id::unique(("details", "variable", *line, name))
            }
        }
    }
}

/// The parts of the app the detail and details popups read.
pub(super) struct DetailContext<'a> {
    pub tabs: &'a [Tab],
    pub active: usize,
    pub outputs: &'a [StatementOutput],
    pub settings: &'a Settings,
    pub registry: &'a Registry,
}

impl DetailContext<'_> {
    pub(super) fn details_ui(&self, ui: &mut Ui, target: &HoverTarget) {
        match target {
            HoverTarget::Entry { glyph, .. } => {
                if let Some(entry) = self.registry.entry_by_glyph(*glyph) {
                    self.entry_details_ui(ui, entry);
                }
            }
            HoverTarget::Variable { line, name, .. } => {
                self.variable_details_ui(ui, *line, name);
            }
        }
    }

    fn entry_details_ui(&self, ui: &mut Ui, entry: &Entry) {
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

    fn variable_details_ui(&self, ui: &mut Ui, line: usize, name: &str) {
        let Some(definition_line) = self.definition_line(line, name) else {
            return;
        };
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let Some(text) = tab.text.split('\n').nth(definition_line) else {
            return;
        };
        let Some(statement) = self.outputs.get(definition_line) else {
            return;
        };
        if statement.source_hash != line_hash(text) {
            return;
        }
        let LineOutput::Value(value) = &statement.output else {
            return;
        };
        let formatted = format_facet(value, self.settings, self.registry);

        ui.heading(name);
        ui.label(RichText::new(format!("{name} = {formatted}")).strong());
        if let Some(value) = &statement.value {
            self.value_details_ui(ui, value);
        }
    }

    fn definition_line(&self, line: usize, name: &str) -> Option<usize> {
        let tab = self.tabs.get(self.active)?;
        tab.text
            .split('\n')
            .enumerate()
            .take(line)
            .find_map(|(index, text)| (definition_name(text) == Some(name)).then_some(index))
    }

    fn value_details_ui(&self, ui: &mut Ui, value: &FacetValue) {
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
        }
        if let Some(process) = &value.process {
            if let Some(extent) = process.extent {
                rows.push((
                    "extent".to_string(),
                    match extent {
                        Extent::Machines(count) => format!("{count} machines"),
                        Extent::MachineSeconds(seconds) => format!("{seconds} machine-seconds"),
                    },
                ));
            }
            for (unit, amount) in &process.net {
                let rate = !matches!(process.extent, Some(Extent::MachineSeconds(_)));
                rows.push((
                    unit_label(self.registry, *unit),
                    format_amount(*amount, *unit, rate, self.settings, self.registry),
                ));
            }
            if let Some(power) = process.power_w {
                rows.push(("power".to_string(), format_power(power, self.settings)));
            }
            if let Some(energy) = process.energy_j {
                rows.push(("energy".to_string(), format_energy(energy, self.settings)));
            }
        }
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
        self.section_grid(ui, "value", rows);
    }

    fn section_grid(&self, ui: &mut Ui, title: &str, rows: Vec<(String, String)>) {
        ui.add_space(6.0);
        ui.label(RichText::new(title).strong());
        self.rows_grid(ui, title, rows);
    }

    fn rows_grid(&self, ui: &mut Ui, id: &str, rows: Vec<(String, String)>) {
        egui::Grid::new(ui.make_persistent_id(("entry_details", id)))
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

    /// The details for `target`, shown next to it.
    pub(super) fn details_popup(&self, ctx: &egui::Context, target: HoverTarget) {
        let anchor = target.rect();
        let pointer = ctx.pointer_hover_pos().unwrap_or(anchor.center());
        let layer = ctx.layer_id_at(pointer).unwrap_or_else(|| {
            egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("details"))
        });
        egui::Tooltip::always_open(ctx.clone(), layer, target.tooltip_id(), anchor)
            .width(360.0)
            .show(|ui| self.details_ui(ui, &target));
    }

    /// Full results for the line under the caret, floating beside it.
    pub(super) fn detail_panel(
        &self,
        ctx: &egui::Context,
        anchor: egui::Pos2,
        line: usize,
        width: f32,
    ) {
        let registry = self.registry;
        let Some(tab) = self.tabs.get(self.active) else {
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
        if let Some(target) = hovered {
            self.details_popup(ctx, target);
        }
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

/// Draws a monospace value and returns the rect of any hovered icon glyph in it.
fn icon_label(ui: &mut Ui, text: &str, registry: &Registry) -> Option<HoverTarget> {
    let font_id = TextStyle::Monospace.resolve(ui.style());
    let color = ui.visuals().text_color();
    let galley = ui
        .painter()
        .layout(text.to_string(), font_id, color, ui.available_width());
    let (rect, _response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley.clone(), color);

    let pointer = ui.ctx().pointer_hover_pos()?;
    hovered_icon(registry, &galley, rect.min, pointer)
        .map(|(_, glyph, rect)| HoverTarget::Entry { glyph, rect })
}

fn detail_ui(
    ui: &mut Ui,
    statement: &StatementOutput,
    settings: &Settings,
    registry: &Registry,
) -> Option<HoverTarget> {
    let mut hovered = None;
    match &statement.output {
        LineOutput::Nothing => {}
        LineOutput::Error(error) => {
            ui.label(RichText::new(&error.message).color(Color32::from_rgb(220, 90, 90)));
        }
        LineOutput::Value(value) => {
            let formatted = format_facet(value, settings, registry);
            hovered = icon_label(ui, &formatted, registry);

            if let Some(process) = &value.process {
                let rate = !matches!(process.extent, Some(Extent::MachineSeconds(_)));
                let mut produces = Vec::new();
                let mut consumes = Vec::new();
                for (unit, amount) in &process.net {
                    if *unit == BaseUnit::Craft {
                        continue;
                    }
                    let text = format_amount(amount.abs(), *unit, rate, settings, registry);
                    if *amount > 0.0 {
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
                            if hovered.is_none() {
                                hovered = icon_label(ui, produce, registry);
                            }
                        }
                    });
                    ui.vertical(|ui| {
                        ui.set_min_width(90.0);
                        ui.label(RichText::new("consumes").weak());
                        for consume in &consumes {
                            if hovered.is_none() {
                                hovered = icon_label(ui, consume, registry);
                            }
                        }
                    });
                });
                if let Some(craft) = process.net.get(&BaseUnit::Craft).copied() {
                    let text = if rate {
                        format!("{} craft/s", format_plain_count(craft, settings, registry))
                    } else {
                        format!("{} craft", format_plain_count(craft, settings, registry))
                    };
                    if hovered.is_none() {
                        hovered = icon_label(ui, &text, registry);
                    }
                }
                if let Some(power) = process.power_w {
                    if hovered.is_none() {
                        hovered = icon_label(ui, &format_power(power, settings), registry);
                    }
                }
                if let Some(energy) = process.energy_j {
                    if hovered.is_none() {
                        hovered = icon_label(ui, &format_energy(energy, settings), registry);
                    }
                }
            }
        }
    }
    hovered
}

fn format_plain_count(value: f64, settings: &Settings, registry: &Registry) -> String {
    format_value(&Value::scalar(value), settings, registry)
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
        LineOutput::Value(value) => Some(format_facet(value, settings, registry)),
    }
}
