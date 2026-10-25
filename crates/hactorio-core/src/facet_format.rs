//! Formatting for facet values.

use hactorio_data::{EntryId, Registry, STACK_SIZE_GLYPH};

use crate::facet_machine;
use crate::facets::{
    Cap, CapacityFacet, ConfigFacet, FacetValue, MachineFacet, Monomial, ProcessFacet, RateFacet,
};
use crate::format::{
    format_amount, format_energy, format_plain_number, format_power, format_value,
};
use crate::settings::Settings;
use crate::value::{BaseUnit, Value, is_nonzero, is_zero};

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
    if let Some(count) = machine.count
        && is_nonzero(count - 1.0)
    {
        out.push_str(&format_plain_number(count, settings));
    }
    out.push(registry.entry_by_index(machine.machine).glyph);
    out.push_str(&format_config_suffix(&machine.config, settings, registry));
    out
}

/// Machine stats for the multi-line result: speed, productivity, total power,
/// and total pollution.
pub fn format_machine_stats(
    machine: &MachineFacet,
    settings: &Settings,
    registry: &Registry,
) -> Result<Vec<(String, String)>, String> {
    let modifiers =
        facet_machine::modifiers(registry, machine.machine, &machine.config, None, settings)?;
    let count = machine.count.unwrap_or(1.0);
    let speed = if is_zero(modifiers.speed_effect) {
        format_plain_number(modifiers.speed, settings)
    } else {
        format!(
            "{} ({})",
            format_plain_number(modifiers.speed, settings),
            format_percent(modifiers.speed_effect, settings)
        )
    };
    let productivity = 1.0 + modifiers.productivity;
    let productivity = if modifiers.productivity_uncapped > modifiers.productivity {
        format!(
            "{} ({} → {})",
            format_plain_number(productivity, settings),
            format_percent(modifiers.productivity_uncapped, settings),
            format_percent(modifiers.productivity, settings)
        )
    } else if is_zero(modifiers.productivity_effect) {
        format_plain_number(productivity, settings)
    } else {
        format!(
            "{} ({})",
            format_plain_number(productivity, settings),
            format_percent(modifiers.productivity_effect, settings)
        )
    };
    let mut power = format_power(modifiers.power_w * count, settings);
    power.push_str(&format_consumption(&modifiers, settings));
    let pollution = format!(
        "{}/m",
        format_plain_number(modifiers.pollution_per_minute * count, settings)
    );
    Ok(vec![
        ("speed".to_string(), speed),
        ("prod".to_string(), productivity),
        ("power".to_string(), power),
        ("pollution".to_string(), pollution),
    ])
}

fn format_percent(fraction: f64, settings: &Settings) -> String {
    let percent = format_plain_number(fraction * 100.0, settings);
    if fraction > 0.0 {
        format!("+{percent}%")
    } else {
        format!("{percent}%")
    }
}

fn format_consumption(modifiers: &facet_machine::Modifiers, settings: &Settings) -> String {
    let original = modifiers.consumption_multiplier;
    if is_zero(original - 1.0) {
        return String::new();
    }
    let clamped = modifiers.clamped_consumption_multiplier();
    let original_text = format_percent(original - 1.0, settings);
    if is_zero(original - clamped) {
        format!(" ({original_text})")
    } else {
        format!(
            " ({original_text} → {})",
            format_percent(clamped - 1.0, settings)
        )
    }
}

/// The config suffix used after machine lines: ` (<config>)`, or empty.
pub fn format_config_suffix(
    config: &ConfigFacet,
    settings: &Settings,
    registry: &Registry,
) -> String {
    if config.is_empty() {
        String::new()
    } else {
        format!(" ({})", format_config(config, settings, registry))
    }
}

pub fn format_config(config: &ConfigFacet, settings: &Settings, registry: &Registry) -> String {
    let mut parts = Vec::new();
    for module in &config.modules {
        parts.push(repeated_glyph(module.count, module.module, registry));
    }
    for group in &config.beacons {
        let mut text = String::new();
        if group.count != 1 {
            text.push_str(&format_plain_number(f64::from(group.count), settings));
        }
        text.push(registry.entry_by_index(group.beacon).glyph);
        for module in &group.modules {
            text.push_str(&repeated_glyph(module.count, module.module, registry));
        }
        parts.push(text);
    }
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join("+")
    }
}

fn repeated_glyph(count: u32, id: EntryId, registry: &Registry) -> String {
    registry
        .entry_by_index(id)
        .glyph
        .to_string()
        .repeat(count as usize)
}

pub fn format_rate(rate: &RateFacet, settings: &Settings, _registry: &Registry) -> String {
    match rate {
        RateFacet::Fluid(value) => format!("{}/s", format_plain_number(*value, settings)),
        RateFacet::Items(terms) => {
            let terms: Vec<String> = ordered_terms(terms)
                .into_iter()
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

/// `Full` first, then `Limit` in descending order.
fn ordered_terms(terms: &std::collections::BTreeMap<Cap, f64>) -> Vec<(&Cap, &f64)> {
    let mut ordered: Vec<(&Cap, &f64)> = terms.iter().collect();
    ordered.sort_by_key(|(cap, _)| match cap {
        Cap::Full => (0, std::cmp::Reverse(0)),
        Cap::Limit(limit) => (1, std::cmp::Reverse(*limit)),
    });
    ordered
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
            let terms: Vec<String> = ordered_terms(terms)
                .into_iter()
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

pub fn format_process(process: &ProcessFacet, settings: &Settings, registry: &Registry) -> String {
    let mut parts = Vec::new();
    let machines = format_process_machines_inline(process, settings, registry);
    if !machines.is_empty() {
        parts.push(machines);
    }
    let rate = !process.extent.is_machine_seconds();
    for (unit, amount) in &process.net {
        if *unit == BaseUnit::Craft {
            continue;
        }
        parts.push(format_amount(*amount, *unit, rate, settings, registry));
    }
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join(" · ")
    }
}

fn format_process_machines_inline(
    process: &ProcessFacet,
    settings: &Settings,
    registry: &Registry,
) -> String {
    let machine_seconds = process.extent.is_machine_seconds();
    let mut parts = Vec::new();
    for (spec, count) in &process.extent.machines {
        let mut text = String::new();
        if is_nonzero(*count - 1.0) {
            text.push_str(&format_plain_number(*count, settings));
            text.push(' ');
        }
        text.push(registry.entry_by_index(spec.machine).glyph);
        if machine_seconds {
            text.push_str("*s");
        }
        parts.push(text);
    }
    parts.join("+")
}

/// Process inputs: negative net rows, with their signed values.
pub fn format_process_inputs(
    process: &ProcessFacet,
    settings: &Settings,
    registry: &Registry,
) -> Vec<String> {
    format_process_rows(process, settings, registry, |amount| amount < 0.0)
}

/// Process outputs: positive net rows, with their signed values.
pub fn format_process_outputs(
    process: &ProcessFacet,
    settings: &Settings,
    registry: &Registry,
) -> Vec<String> {
    format_process_rows(process, settings, registry, |amount| amount > 0.0)
}

fn format_process_rows(
    process: &ProcessFacet,
    settings: &Settings,
    registry: &Registry,
    keep: impl Fn(f64) -> bool,
) -> Vec<String> {
    let rate = !process.extent.is_machine_seconds();
    process
        .net
        .iter()
        .filter(|(unit, _)| **unit != BaseUnit::Craft)
        .filter(|(_, amount)| keep(**amount))
        .map(|(unit, amount)| format_amount(amount.abs(), *unit, rate, settings, registry))
        .collect()
}

/// One entry per machine type/config for the multi-line machines section.
pub fn format_process_machine_lines(
    process: &ProcessFacet,
    settings: &Settings,
    registry: &Registry,
) -> Vec<String> {
    let machine_seconds = process.extent.is_machine_seconds();
    process
        .extent
        .machines
        .iter()
        .map(|(spec, count)| {
            let mut text = String::new();
            if is_nonzero(*count - 1.0) {
                text.push_str(&format_plain_number(*count, settings));
                text.push(' ');
            }
            text.push(registry.entry_by_index(spec.machine).glyph);
            if machine_seconds {
                text.push_str("*s");
            }
            text.push_str(&format_config_suffix(&spec.config, settings, registry));
            text
        })
        .collect()
}

/// Total craft for the multi-line process header.
pub fn format_process_craft(process: &ProcessFacet, settings: &Settings) -> String {
    let rate = !process.extent.is_machine_seconds();
    let craft = process.net.get(&BaseUnit::Craft).copied().unwrap_or(0.0);
    if rate {
        format!("{} craft/s", format_plain_number(craft, settings))
    } else {
        format!("{} craft", format_plain_number(craft, settings))
    }
}

/// Total energy or power for the bottom of the inputs section.
pub fn format_process_energy(process: &ProcessFacet, settings: &Settings) -> Option<String> {
    if let Some(energy) = process.energy_j {
        Some(format_energy(energy, settings))
    } else {
        process.power_w.map(|power| format_power(power, settings))
    }
}

/// A convenience for the detail pane: the item facet as a display value.
pub fn item_value(item: &Monomial) -> Value {
    to_value(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facets::{ConfigFacet, Extent, MachineSpec, ModuleCount, ProcessFacet};
    use hactorio_data::{Locale, postprocess};
    use std::collections::BTreeMap;

    const MACHINE_DUMP: &str = r#"{
      "item": {
        "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100},
        "iron-gear-wheel": {"type": "item", "name": "iron-gear-wheel", "stack_size": 100},
        "assembling-machine": {"type": "item", "name": "assembling-machine", "stack_size": 50},
        "prod-module": {"type": "module", "name": "prod-module", "category": "productivity", "tier": 3, "stack_size": 50, "effect": {"productivity": 0.1, "consumption": -0.03}},
        "speed-module": {"type": "module", "name": "speed-module", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}},
        "beacon": {"type": "item", "name": "beacon", "stack_size": 20}
      },
      "fluid": {},
      "module": {
        "prod-module": {"type": "module", "name": "prod-module", "category": "productivity", "tier": 3, "stack_size": 50, "effect": {"productivity": 0.1, "consumption": -0.03}},
        "speed-module": {"type": "module", "name": "speed-module", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}}
      },
      "assembling-machine": {
        "assembling-machine": {"type": "assembling-machine", "name": "assembling-machine", "crafting_categories": ["crafting"], "crafting_speed": 2.333, "module_slots": 4, "allowed_effects": ["speed", "productivity", "consumption"], "energy_usage": "100kW", "energy_source": {"emissions_per_minute": {"pollution": 10}}}
      },
      "beacon": {
        "beacon": {"type": "beacon", "name": "beacon", "module_slots": 2, "allowed_effects": ["speed", "productivity"], "distribution_effectivity": 1.0, "profile": [1.0], "energy_usage": "480kW"}
      }
    }"#;

    fn machine_registry() -> Registry {
        postprocess(MACHINE_DUMP, &Locale::empty()).unwrap()
    }

    fn glyph(registry: &Registry, id: &str) -> String {
        registry.entry(id).unwrap().glyph.to_string()
    }

    #[test]
    fn config_display_repeats_modules_and_keeps_beacon_counts() {
        let registry = machine_registry();
        let settings = Settings::default();
        let module = registry.entry_id("speed-module").unwrap();
        let beacon = registry.entry_id("beacon").unwrap();
        let config = ConfigFacet {
            modules: vec![ModuleCount { module, count: 4 }],
            beacons: vec![crate::facets::BeaconGroup {
                beacon,
                count: 4,
                modules: vec![ModuleCount { module, count: 2 }],
            }],
        };
        let module_glyph = glyph(&registry, "speed-module");
        let beacon_glyph = glyph(&registry, "beacon");
        assert_eq!(
            format_config(&config, &settings, &registry),
            format!(
                "{module_glyph}{module_glyph}{module_glyph}{module_glyph}+4{beacon_glyph}{module_glyph}{module_glyph}"
            )
        );
        assert_eq!(
            format_config_suffix(&config, &settings, &registry),
            format!(" ({})", format_config(&config, &settings, &registry))
        );
        assert_eq!(
            format_config_suffix(&ConfigFacet::default(), &settings, &registry),
            ""
        );
    }

    #[test]
    fn machine_display_uses_config_suffix_and_stats() {
        let registry = machine_registry();
        let settings = Settings::default();
        let machine = registry.entry_id("assembling-machine").unwrap();
        let module = registry.entry_id("prod-module").unwrap();
        let config = ConfigFacet {
            modules: vec![ModuleCount { module, count: 1 }],
            beacons: Vec::new(),
        };
        let facet = MachineFacet {
            machine,
            config,
            count: Some(2.0),
        };
        assert_eq!(
            format_machine(&facet, &settings, &registry),
            format!(
                "2{}{}",
                glyph(&registry, "assembling-machine"),
                format_config_suffix(&facet.config, &settings, &registry)
            )
        );
        let stats = format_machine_stats(&facet, &settings, &registry).unwrap();
        assert_eq!(stats[0].0, "speed");
        assert_eq!(stats[0].1, "2.333");
        assert_eq!(stats[1].0, "prod");
        assert_eq!(stats[1].1, "1.1 (+10%)");
        assert_eq!(stats[2].0, "power");
        assert!(stats[2].1.contains("(-3%)"), "{}", stats[2].1);
        assert_eq!(stats[3].0, "pollution");
        assert!(stats[3].1.ends_with("/m"), "{}", stats[3].1);

        let clamped = MachineFacet {
            machine,
            config: ConfigFacet {
                modules: vec![ModuleCount { module, count: 35 }],
                beacons: Vec::new(),
            },
            count: Some(1.0),
        };
        let stats = format_machine_stats(&clamped, &settings, &registry).unwrap();
        assert_eq!(stats[1].1, "4 (+350% → +300%)");
        assert!(stats[2].1.contains("(-105% → -80%)"), "{}", stats[2].1);
    }

    #[test]
    fn process_display_uses_machine_specs_and_sections() {
        let registry = machine_registry();
        let settings = Settings::default();
        let machine = registry.entry_id("assembling-machine").unwrap();
        let empty = MachineSpec {
            machine,
            config: ConfigFacet::default(),
        };
        let configured = MachineSpec {
            machine,
            config: ConfigFacet {
                modules: vec![ModuleCount {
                    module: registry.entry_id("prod-module").unwrap(),
                    count: 1,
                }],
                beacons: Vec::new(),
            },
        };
        let mut extent = Extent::machines(configured.clone(), 2.0);
        extent.machines.insert(empty.clone(), 1.0);
        let process = ProcessFacet {
            extent,
            net: BTreeMap::from([
                (
                    BaseUnit::Item(registry.entry_id("iron-gear-wheel").unwrap()),
                    1.0,
                ),
                (
                    BaseUnit::Item(registry.entry_id("iron-plate").unwrap()),
                    -2.0,
                ),
                (BaseUnit::Craft, 1.0),
            ]),
            power_w: Some(300_000.0),
            energy_j: None,
        };
        let machine_glyph = glyph(&registry, "assembling-machine");
        let suffix = format_config_suffix(&configured.config, &settings, &registry);
        let single = format_process(&process, &settings, &registry);
        assert!(single.contains(&format!("2 {machine_glyph}")), "{single}");
        assert!(single.contains('+'), "{single}");
        assert!(!single.contains("craft"), "{single}");
        assert!(!single.contains("kW"), "{single}");
        assert!(single.contains(" · "), "{single}");

        let inputs = format_process_inputs(&process, &settings, &registry);
        assert_eq!(inputs.len(), 1);
        assert!(!inputs[0].starts_with('-'), "{inputs:?}");
        let outputs = format_process_outputs(&process, &settings, &registry);
        assert_eq!(outputs.len(), 1);
        assert!(!outputs[0].starts_with('-'), "{outputs:?}");

        let machines = format_process_machine_lines(&process, &settings, &registry);
        assert_eq!(
            machines,
            vec![machine_glyph.clone(), format!("2 {machine_glyph}{suffix}")]
        );
        assert_eq!(format_process_craft(&process, &settings), "1 craft/s");
        assert_eq!(
            format_process_energy(&process, &settings).as_deref(),
            Some("300 kW")
        );

        let machine_seconds = ProcessFacet {
            extent: Extent::machine_seconds(empty, 2.0),
            ..process.clone()
        };
        assert_eq!(
            format_process_machine_lines(&machine_seconds, &settings, &registry),
            vec![format!("2 {machine_glyph}*s")]
        );
    }

    #[test]
    fn rate_and_capacity_terms_are_ordered_full_then_descending_limit() {
        let settings = Settings::default();
        let registry = hactorio_data::postprocess("{}", &hactorio_data::Locale::empty()).unwrap();
        let rate = RateFacet::Items(BTreeMap::from([
            (Cap::Limit(1), 10.0),
            (Cap::Full, 30.0),
            (Cap::Limit(4), 20.0),
            (Cap::Limit(2), 15.0),
        ]));
        assert_eq!(
            format_rate(&rate, &settings, &registry),
            format!(
                "30{STACK_SIZE_GLYPH}/s + 20{STACK_SIZE_GLYPH}₄/s + 15{STACK_SIZE_GLYPH}₂/s + 10/s"
            )
        );
        let capacity = CapacityFacet::Items(BTreeMap::from([
            (Cap::Limit(1), 10.0),
            (Cap::Full, 30.0),
            (Cap::Limit(4), 20.0),
        ]));
        assert_eq!(
            format_capacity(&capacity, &settings, &registry),
            format!("30{STACK_SIZE_GLYPH} + 20{STACK_SIZE_GLYPH}₄ + 10")
        );
    }
}
