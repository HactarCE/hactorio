//! Focused-line breakdown: everything that went into an output.

use std::collections::{BTreeMap, BTreeSet};

use hactorio_data::{EntryId, PrototypeKind, Registry};

use crate::config;
use crate::eval::{Eval, MachineValue, RecipeQuantity, RecipeValue};
use crate::format::{
    format_amount, format_energy, format_plain_number, format_power, format_value,
};
use crate::settings::Settings;
use crate::value::{BaseUnit, Value};

/// The side-panel detail for the focused line.
#[derive(Debug, Clone, PartialEq)]
pub struct Breakdown {
    pub sections: Vec<BreakdownSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BreakdownSection {
    pub title: String,
    pub rows: Vec<BreakdownRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BreakdownRow {
    pub label: String,
    pub value: String,
}

/// Builds the breakdown for an evaluated line.
///
/// Scalars have no breakdown; their value is already the whole answer.
pub fn breakdown(value: &Eval, settings: &Settings, registry: &Registry) -> Breakdown {
    match value {
        Eval::Scalar(_) => Breakdown {
            sections: Vec::new(),
        },
        Eval::Machine(machine) => machine_breakdown(machine, settings, registry),
        Eval::Recipe(recipe) => recipe_breakdown(recipe, settings, registry),
    }
}

fn machine_breakdown(value: &MachineValue, settings: &Settings, registry: &Registry) -> Breakdown {
    let mut rows = vec![
        row("machine", entry_label(registry, value.machine)),
        row("count", format_plain_number(value.count, settings)),
        row("modules", modules_label(&value.modules, registry, settings)),
    ];

    if let Some(machine) = registry.entry_by_index(value.machine).machine() {
        let modifiers = config::modifiers(registry, value.machine, &value.modules, None);
        rows.push(row(
            "slots",
            format!(
                "{} / {}",
                module_count(&value.modules),
                machine.module_slots
            ),
        ));
        rows.push(row(
            "base speed",
            format_plain_number(machine.speed, settings),
        ));
        rows.push(row("speed", format_plain_number(modifiers.speed, settings)));
    }

    Breakdown {
        sections: vec![BreakdownSection {
            title: "machine".to_string(),
            rows,
        }],
    }
}

fn recipe_breakdown(value: &RecipeValue, settings: &Settings, registry: &Registry) -> Breakdown {
    let recipe = registry.recipe(value.recipe);
    let Some(machine) = registry.entry_by_index(value.machine).machine() else {
        return Breakdown {
            sections: Vec::new(),
        };
    };
    let modifiers = config::modifiers(registry, value.machine, &value.modules, Some(recipe));
    let rate = matches!(value.quantity, RecipeQuantity::Machines(_));
    let multiplier = value.quantity.magnitude();
    let crafts_per_second = modifiers.speed / recipe.craft_time;

    let mut machine_rows = vec![
        row("machine", entry_label(registry, value.machine)),
        row("modules", modules_label(&value.modules, registry, settings)),
        row(
            "slots",
            format!(
                "{} / {}",
                module_count(&value.modules),
                machine.module_slots
            ),
        ),
        row("base speed", format_plain_number(machine.speed, settings)),
        row("speed", format_plain_number(modifiers.speed, settings)),
    ];
    if rate {
        machine_rows.push(row("count", format_plain_number(multiplier, settings)));
    } else {
        machine_rows.push(row(
            "machine time",
            format_value(&Value::seconds(multiplier), settings, registry),
        ));
    }
    machine_rows.push(row(
        "productivity",
        format_percent(modifiers.productivity, settings),
    ));
    machine_rows.push(row(
        "consumption",
        format_percent(modifiers.consumption_multiplier - 1.0, settings),
    ));

    let recipe_rows = vec![
        row("recipe", recipe_label(registry, value.recipe)),
        row("categories", categories_label(&recipe.categories)),
        row(
            "craft time",
            format_value(&Value::seconds(recipe.craft_time), settings, registry),
        ),
        row(
            "productivity allowed",
            if recipe.allow_productivity {
                "yes"
            } else {
                "no"
            }
            .to_string(),
        ),
    ];

    let mut produced: BTreeMap<BaseUnit, f64> = BTreeMap::new();
    let mut consumed: BTreeMap<BaseUnit, f64> = BTreeMap::new();
    for product in &recipe.products {
        if let Some(unit) = base_unit(registry, product.kind, &product.name) {
            let amount = product.amount
                + modifiers.productivity * (product.amount - product.ignored_by_productivity);
            *produced.entry(unit).or_insert(0.0) += crafts_per_second * amount * multiplier;
        }
    }
    for ingredient in &recipe.ingredients {
        if let Some(unit) = base_unit(registry, ingredient.kind, &ingredient.name) {
            *consumed.entry(unit).or_insert(0.0) +=
                crafts_per_second * ingredient.amount * multiplier;
        }
    }

    let units: BTreeSet<BaseUnit> = produced.keys().chain(consumed.keys()).copied().collect();
    let mut item_rows = Vec::new();
    for unit in units {
        let produced_here = produced.get(&unit).copied().unwrap_or(0.0);
        let consumed_here = consumed.get(&unit).copied().unwrap_or(0.0);
        let net = produced_here - consumed_here;
        let mut parts = Vec::new();
        if produced_here != 0.0 {
            parts.push(format!(
                "produced {}",
                format_amount(produced_here, unit, rate, settings, registry)
            ));
        }
        if consumed_here != 0.0 {
            parts.push(format!(
                "consumed {}",
                format_amount(consumed_here, unit, rate, settings, registry)
            ));
        }
        parts.push(format!(
            "net {}",
            format_amount(net, unit, rate, settings, registry)
        ));
        item_rows.push(BreakdownRow {
            label: unit_label(registry, unit),
            value: parts.join(", "),
        });
    }

    let mut power_rows = vec![row(
        "base usage",
        format_power(machine.energy_usage_w, settings),
    )];
    power_rows.push(row(
        "per machine",
        format_power(modifiers.power_w, settings),
    ));
    if rate {
        power_rows.push(row(
            "total power",
            format_power(modifiers.power_w * multiplier, settings),
        ));
    }
    power_rows.push(row(
        "energy per craft",
        format_energy(modifiers.power_w * recipe.craft_time, settings),
    ));
    if !rate {
        power_rows.push(row(
            "total energy",
            format_energy(modifiers.power_w * multiplier, settings),
        ));
    }

    let per_machine_pollution = modifiers.pollution_per_minute;
    let total_pollution = if rate {
        per_machine_pollution * multiplier
    } else {
        per_machine_pollution * multiplier / 60.0
    };
    let pollution_rows = vec![
        row(
            "per machine",
            format!(
                "{}/min",
                format_plain_number(per_machine_pollution, settings)
            ),
        ),
        row(
            "total",
            format!("{}/min", format_plain_number(total_pollution, settings)),
        ),
    ];

    let mut note_rows = Vec::new();
    for product in &recipe.products {
        if product.ignored_by_productivity > 0.0 {
            note_rows.push(row(
                "ignored by productivity",
                format!(
                    "{} of {} {}",
                    format_plain_number(product.ignored_by_productivity, settings),
                    format_plain_number(product.amount, settings),
                    entry_label_for_name(registry, product.kind, &product.name),
                ),
            ));
        }
    }

    let mut sections = vec![
        section("machine", machine_rows),
        section("recipe", recipe_rows),
    ];
    if !item_rows.is_empty() {
        sections.push(section("items", item_rows));
    }
    sections.push(section("power", power_rows));
    sections.push(section("pollution", pollution_rows));
    if !note_rows.is_empty() {
        sections.push(section("notes", note_rows));
    }

    Breakdown { sections }
}

fn section(title: &str, rows: Vec<BreakdownRow>) -> BreakdownSection {
    BreakdownSection {
        title: title.to_string(),
        rows,
    }
}

fn row(label: &str, value: impl Into<String>) -> BreakdownRow {
    BreakdownRow {
        label: label.to_string(),
        value: value.into(),
    }
}

fn module_count(modules: &[crate::eval::ModuleCount]) -> u32 {
    modules.iter().map(|module| module.count as u32).sum()
}

fn modules_label(
    modules: &[crate::eval::ModuleCount],
    registry: &Registry,
    settings: &Settings,
) -> String {
    if modules.is_empty() {
        return "none".to_string();
    }
    modules
        .iter()
        .map(|module| {
            format!(
                "{} × {}",
                entry_label(registry, module.module),
                format_plain_number(module.count, settings)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_percent(value: f64, settings: &Settings) -> String {
    format!("{}%", format_plain_number(value * 100.0, settings))
}

fn categories_label(categories: &[Box<str>]) -> String {
    categories
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(", ")
}

fn entry_label(registry: &Registry, id: EntryId) -> String {
    let entry = registry.entry_by_index(id);
    match &entry.display_name {
        Some(name) => format!("{} {name}", entry.glyph),
        None => format!("{} {}", entry.glyph, entry.id),
    }
}

fn entry_label_for_name(registry: &Registry, kind: PrototypeKind, name: &str) -> String {
    match base_unit(registry, kind, name) {
        Some(unit) => unit_label(registry, unit),
        None => name.to_string(),
    }
}

fn unit_label(registry: &Registry, unit: BaseUnit) -> String {
    match unit {
        BaseUnit::Item(id) | BaseUnit::Fluid(id) => entry_label(registry, id),
        BaseUnit::Second => "second".to_string(),
        BaseUnit::Joule => "joule".to_string(),
        BaseUnit::Craft => "craft".to_string(),
    }
}

fn recipe_label(registry: &Registry, id: hactorio_data::RecipeId) -> String {
    let recipe = registry.recipe(id);
    match &recipe.display_name {
        Some(name) => name.to_string(),
        None => recipe.id.to_string(),
    }
}

fn base_unit(registry: &Registry, kind: PrototypeKind, name: &str) -> Option<BaseUnit> {
    let id = registry.entry_id(name)?;
    Some(match kind {
        PrototypeKind::Item => BaseUnit::Item(id),
        PrototypeKind::Fluid => BaseUnit::Fluid(id),
    })
}
