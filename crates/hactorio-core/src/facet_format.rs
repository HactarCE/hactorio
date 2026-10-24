//! Formatting for facet values.

use hactorio_data::{EntryId, Registry, STACK_SIZE_GLYPH};

use crate::facets::{
    Cap, CapacityFacet, Config, Extent, FacetValue, MachineFacet, Monomial, Process, RateFacet,
};
use crate::format::{
    format_amount, format_energy, format_plain_number, format_power, format_value,
};
use crate::settings::Settings;
use crate::value::{BaseUnit, Value};

/// Formats the first displayable facet: item, machine, process, config, then
/// rate, capacity, recipe.
pub fn format_facet(value: &FacetValue, settings: &Settings, registry: &Registry) -> String {
    if let Some(item) = &value.item {
        return format_value(&to_value(item), settings, registry);
    }
    if let Some(machine) = &value.machine {
        return format_machine(machine, settings, registry);
    }
    if let Some(process) = &value.process {
        return format_process(process, settings, registry);
    }
    if let Some(config) = &value.config {
        return if config.is_empty() {
            "0".to_string()
        } else {
            format_config(config, settings, registry)
        };
    }
    if let Some(rate) = &value.rate {
        return format_rate(rate, settings, registry);
    }
    if let Some(capacity) = &value.capacity {
        return format_capacity(capacity, settings, registry);
    }
    if let Some(recipe) = value.recipe {
        return registry
            .entry_by_index(recipe_entry(registry, recipe))
            .glyph
            .to_string();
    }
    "0".to_string()
}

fn recipe_entry(registry: &Registry, recipe: hactorio_data::RecipeId) -> EntryId {
    registry
        .entry_id(registry.recipe(recipe).id.as_ref())
        .unwrap_or(EntryId::from_index(0))
}

fn to_value(item: &Monomial) -> Value {
    Value::new(
        item.coef(),
        item.units()
            .iter()
            .map(|(unit, exponent)| (*unit, *exponent)),
    )
}

pub fn format_machine(machine: &MachineFacet, settings: &Settings, registry: &Registry) -> String {
    let mut out = String::new();
    if let Some(count) = machine.count {
        if count != 1.0 {
            out.push_str(&format_plain_number(count, settings));
        }
    }
    out.push_str(&registry.entry_by_index(machine.machine).glyph.to_string());
    if !machine.config.is_empty() {
        out.push_str(" with ");
        out.push_str(&format_config(&machine.config, settings, registry));
    }
    out
}

pub fn format_config(config: &Config, settings: &Settings, registry: &Registry) -> String {
    let mut parts = Vec::new();
    for module in &config.modules {
        parts.push(format_counted(
            module.count,
            module.module,
            settings,
            registry,
        ));
    }
    for group in &config.beacons {
        let mut text = format_counted(group.count, group.beacon, settings, registry);
        for module in &group.modules {
            text.push(' ');
            text.push_str(&format_counted(
                module.count,
                module.module,
                settings,
                registry,
            ));
        }
        parts.push(text);
    }
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join(" + ")
    }
}

fn format_counted(count: u32, id: EntryId, settings: &Settings, registry: &Registry) -> String {
    let glyph = registry.entry_by_index(id).glyph;
    if count == 1 {
        glyph.to_string()
    } else {
        format!(
            "{}{}",
            format_plain_number(f64::from(count), settings),
            glyph
        )
    }
}

pub fn format_rate(rate: &RateFacet, settings: &Settings, _registry: &Registry) -> String {
    match rate {
        RateFacet::Fluid(value) => format!("{}/s", format_plain_number(*value, settings)),
        RateFacet::Items(terms) => {
            let terms: Vec<String> = terms
                .iter()
                .map(|(cap, amount)| {
                    format!(
                        "{}{}/s",
                        format_plain_number(*amount, settings),
                        format_cap(*cap)
                    )
                })
                .collect();
            if terms.is_empty() {
                "0/s".to_string()
            } else {
                terms.join(" + ")
            }
        }
    }
}

pub fn format_capacity(
    capacity: &CapacityFacet,
    settings: &Settings,
    registry: &Registry,
) -> String {
    let _ = registry;
    match capacity {
        CapacityFacet::Fluid(value) => format_plain_number(*value, settings),
        CapacityFacet::Items(terms) => {
            let terms: Vec<String> = terms
                .iter()
                .map(|(cap, amount)| {
                    format!(
                        "{}{}",
                        format_plain_number(*amount, settings),
                        format_cap(*cap)
                    )
                })
                .collect();
            if terms.is_empty() {
                "0".to_string()
            } else {
                terms.join(" + ")
            }
        }
    }
}

fn format_cap(cap: Cap) -> String {
    match cap {
        Cap::Full => STACK_SIZE_GLYPH.to_string(),
        Cap::Limit(1) => String::new(),
        Cap::Limit(2) => format!("{STACK_SIZE_GLYPH}₂"),
        Cap::Limit(3) => format!("{STACK_SIZE_GLYPH}₃"),
        Cap::Limit(4) => format!("{STACK_SIZE_GLYPH}₄"),
        Cap::Limit(n) => format!("{STACK_SIZE_GLYPH}_{n}"),
    }
}

pub fn format_process(process: &Process, settings: &Settings, registry: &Registry) -> String {
    let mut parts = Vec::new();
    if let Some(extent) = process.extent {
        match extent {
            Extent::Machines(count) => {
                parts.push(format!("{} machines", format_plain_number(count, settings)));
            }
            Extent::MachineSeconds(seconds) => {
                parts.push(format!(
                    "{} machine-seconds",
                    format_plain_number(seconds, settings)
                ));
            }
        }
    }
    for (unit, amount) in &process.net {
        if *unit == BaseUnit::Craft {
            continue;
        }
        let rate = !matches!(process.extent, Some(Extent::MachineSeconds(_)));
        parts.push(format_amount(*amount, *unit, rate, settings, registry));
    }
    if let Some(craft) = process.net.get(&BaseUnit::Craft) {
        let rate = !matches!(process.extent, Some(Extent::MachineSeconds(_)));
        parts.push(if rate {
            format!("{} craft/s", format_plain_number(*craft, settings))
        } else {
            format!("{} craft", format_plain_number(*craft, settings))
        });
    }
    if let Some(power) = process.power_w {
        parts.push(format_power(power, settings));
    }
    if let Some(energy) = process.energy_j {
        parts.push(format_energy(energy, settings));
    }
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join(" | ")
    }
}

/// A convenience for the detail pane: the item facet as a display value.
pub fn item_value(item: &Monomial) -> Value {
    to_value(item)
}
