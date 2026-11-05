//! Rendering values as text (§7).

use hactorio_data::{Registry, STACK_SIZE_GLYPH};

use hactorio_data::EntryId;

use super::process::Rows;
use crate::belt::{belt_capacity, BeltTier};

use super::value::{
    Amount, Config, Extent, Machine, MultiValue, Process, Quality, Quantity, Resource, ResourceRef,
    Stacks, Value,
};
use crate::{Settings, TimeUnit};

pub fn format_value(value: &Value, settings: &Settings, registry: &Registry) -> String {
    match value {
        Value::Quality(quality) => quality.glyph().to_string(),
        Value::Process(process) => format_process(process, settings, registry),
        Value::Multi(multi) => format_multi(multi, settings, registry),
    }
}

/// The first facet in canonical order (§7.1). The panel adds the rest.
fn format_multi(multi: &MultiValue, settings: &Settings, registry: &Registry) -> String {
    if let Some(machine) = &multi.machine {
        return format_machine(machine, settings, registry);
    }
    if let Some(quantity) = &multi.quantity {
        return format_quantity(quantity, settings, registry);
    }
    if let Some(stacks) = &multi.stack_rate {
        return format_stacks(stacks, true, settings);
    }
    if let Some(stacks) = &multi.stack_capacity {
        return format_stacks(stacks, false, settings);
    }
    if let Some(config) = &multi.config {
        if config.is_empty() {
            return "0".to_string();
        }
        return format_config(config, registry);
    }
    if let Some(resource) = &multi.resource {
        return format_resource_ref(resource, registry);
    }
    if let Some(recipe) = &multi.recipe {
        let qualifier = match recipe.quality {
            Some(quality) => quality.glyph().to_string(),
            None => String::new(),
        };
        return match recipe_glyph(registry, recipe.id) {
            Some(glyph) => format!("{qualifier}{glyph}"),
            None => format!("{qualifier}recipe#{}", recipe.id.index()),
        };
    }
    "0".to_string()
}

/// Every facet of a value, one line each, in the canonical order of §7.1.
/// This is what the multiline result panel and the token tip show.
pub fn format_facets(value: &Value, settings: &Settings, registry: &Registry) -> Vec<String> {
    match value {
        Value::Quality(quality) => vec![quality.glyph().to_string()],
        Value::Process(process) => format_process_panel(process, settings, registry),
        Value::Multi(multi) => {
            let mut lines = Vec::new();
            if let Some(machine) = &multi.machine {
                lines.push(format_machine(machine, settings, registry));
                if let Some(modifiers) =
                    super::process::machine_modifiers(machine, registry, settings)
                {
                    lines.push(format!(
                        "power {}",
                        format_si(modifiers.power_w * machine.count(), "W", settings)
                    ));
                    lines.push(format!(
                        "speed {}",
                        format_number_with(modifiers.speed, settings)
                    ));
                    lines.push(format!(
                        "productivity {}",
                        format_number_with(1.0 + modifiers.productivity, settings)
                    ));
                }
            }
            if let Some(quantity) = &multi.quantity {
                lines.push(format_quantity(quantity, settings, registry));
            }
            if let Some(config) = &multi.config {
                lines.push(if config.is_empty() {
                    "0".to_string()
                } else {
                    format_config(config, registry)
                });
            }
            if let Some(stacks) = &multi.stack_rate {
                lines.push(format_stacks(stacks, true, settings));
            }
            if let Some(stacks) = &multi.stack_capacity {
                lines.push(format_stacks(stacks, false, settings));
            }
            if let Some(resource) = &multi.resource {
                lines.push(format_resource_ref(resource, registry));
            }
            if let Some(recipe) = &multi.recipe {
                let qualifier = match recipe.quality {
                    Some(quality) => format!(":{}:", quality.name()),
                    None => String::new(),
                };
                lines.push(match recipe_glyph(registry, recipe.id) {
                    Some(glyph) => format!("{qualifier}{glyph}"),
                    None => format!("{qualifier}recipe#{}", recipe.id.index()),
                });
            }
            lines
        }
    }
}

/// The multiline result panel for a process (§7.1, §7.7).
///
/// A summary line carries the power and pollution, then one line per nonzero
/// row with its machine, its scale, and its craft rate or total. Inputs and
/// outputs are aggregated over the whole process, one line per resource.
pub fn format_process_panel(
    process: &Process,
    settings: &Settings,
    registry: &Registry,
) -> Vec<String> {
    let machine_seconds = matches!(process.extent, Extent::MachineSeconds);
    let unit = time_unit_name(settings.time_unit);
    let suffix = if machine_seconds {
        String::new()
    } else {
        format!("/{unit}")
    };
    let factor = if machine_seconds {
        1.0
    } else {
        settings.time_unit.seconds()
    };
    let mut lines = Vec::new();

    // Energy for finished work, power for continuous production (§6.6).
    if let Some(watts) = super::process::power(process, registry, settings) {
        lines.push(if machine_seconds {
            format!("energy {}", format_si(watts, "J", settings))
        } else {
            format!("power {}", format_si(watts, "W", settings))
        });
    }
    if let Some(pollution) = super::process::pollution(process, registry, settings, machine_seconds)
    {
        lines.push(if machine_seconds {
            format!("pollution {}", format_number_with(pollution, settings))
        } else {
            format!("pollution {}/m", format_number_with(pollution, settings))
        });
    }

    for (key, scale) in &process.rows {
        if super::value::is_zero(*scale) {
            continue;
        }
        let crafts = super::process::row_rates(key, registry, settings)
            .map(|rates| rates.craft_per_second * scale)
            .unwrap_or(0.0);
        let crafts = if machine_seconds {
            format!("{} craft", format_number_with(crafts, settings))
        } else {
            format!(
                "{} craft/{}",
                format_number_with(crafts, settings),
                settings.time_unit.short_name()
            )
        };
        lines.push(format!(
            "{} · {crafts}",
            format_row_machine(key, *scale, machine_seconds, settings, registry)
        ));
    }

    for (header, which) in [("in", Rows::Inputs), ("out", Rows::Outputs)] {
        let totals = super::process::totals(process, which);
        if totals.is_empty() {
            continue;
        }
        lines.push(header.to_string());
        for (resource, amount) in totals {
            if !machine_seconds
                && let Some(text) = format_belt_rate(amount, resource, settings, registry)
            {
                lines.push(text);
                continue;
            }
            lines.push(format!(
                "{}{}{suffix}",
                format_prefixed(amount * factor, settings, settings.metric_prefixes),
                format_resource(resource, registry)
            ));
        }
    }

    lines
}

/// A row's machine with its scale in front: `288<foundry>`, or
/// `53.333s*<foundry>` when the scale is machine-seconds.
fn format_row_machine(
    key: &super::value::RowKey,
    scale: f64,
    machine_seconds: bool,
    settings: &Settings,
    registry: &Registry,
) -> String {
    let glyph = registry.entry_by_index(key.machine).glyph;
    let quality = match key.machine_quality {
        Quality::Normal => String::new(),
        quality => format!(":{}:", quality.name()),
    };
    let config = if key.config.is_empty() {
        String::new()
    } else {
        format!("({})", format_config(&key.config, registry))
    };
    let number = format_number_with(scale, settings);
    if machine_seconds {
        format!("{number}s*{quality}{glyph}{config}")
    } else {
        format!("{number}{quality}{glyph}{config}")
    }
}

/// Machines, then the net rows, separated by ` · ` (§7.7).
fn format_process(process: &Process, settings: &Settings, registry: &Registry) -> String {
    let machine_seconds = matches!(process.extent, Extent::MachineSeconds);
    let machines = format_number_with(process.machines(), settings);
    let mut parts = vec![if machine_seconds {
        format!("{machines}*s")
    } else {
        machines
    }];
    let unit = if machine_seconds {
        String::new()
    } else {
        format!("/{}", time_unit_name(settings.time_unit))
    };
    {
        let totals = super::process::totals(process, Rows::Net);
        let factor = if machine_seconds {
            1.0
        } else {
            settings.time_unit.seconds()
        };
        for (resource, amount) in totals {
            if !machine_seconds
                && let Some(text) = format_belt_rate(amount, resource, settings, registry)
            {
                parts.push(text);
                continue;
            }
            parts.push(format!(
                "{}{}{unit}",
                format_prefixed(amount * factor, settings, settings.metric_prefixes),
                format_resource(resource, registry)
            ));
        }
    }
    parts.join(" · ")
}

/// An item rate in belt mode is shown as a count of one belt tier (§7.8).
///
/// The rate is items per second; the tier's belt carries `belt_capacity` items
/// per second, which the stacking research level and the item's stack size
/// decide.
fn format_belt_rate(
    amount_per_second: f64,
    resource: Resource,
    settings: &Settings,
    registry: &Registry,
) -> Option<String> {
    let tier: BeltTier = settings.item_rate_belt_display?;
    let Resource::Item(item) = resource else {
        return None;
    };
    let stack_size = registry.entry_by_index(item.id).item.as_ref()?.stack_size;
    let capacity = belt_capacity(
        tier,
        settings.stacked_belts_display,
        stack_size,
        settings.belt_stacking,
    );
    let glyph = registry.entry(tier.item_id())?.glyph;
    let belt = if settings.stacked_belts_display {
        format!("{STACK_SIZE_GLYPH}{glyph}")
    } else {
        glyph.to_string()
    };
    Some(format!(
        "{} {belt} {}",
        format_number_with(amount_per_second / capacity, settings),
        registry.entry_by_index(item.id).glyph
    ))
}

fn format_machine(machine: &Machine, settings: &Settings, registry: &Registry) -> String {
    let glyph = registry.entry_by_index(machine.entry).glyph;
    let quality = match machine.quality {
        Quality::Normal => String::new(),
        quality => quality.glyph().to_string(),
    };
    // An absent count means "one machine that was not counted", which is a
    // different value from `1:foundry:` (§6.3), so a count is printed whenever
    // it exists, one included.
    let count = match machine.count {
        Some(count) => format!("{}{quality}{glyph}", format_number_with(count, settings)),
        None => format!("{quality}{glyph}"),
    };
    if machine.config.is_empty() {
        count
    } else {
        format!("{count}({})", format_config(&machine.config, registry))
    }
}

/// No spaces and no parentheses: modules repeat, beacon groups carry a count,
/// and groups join with `+` (§7.5).
fn format_config(config: &Config, registry: &Registry) -> String {
    let mut text = String::new();
    for ((module, quality), count) in &config.modules {
        text.push_str(&format_module_glyph(*module, *quality, *count, registry));
    }
    for (group, count) in &config.beacons {
        if !text.is_empty() {
            text.push('+');
        }
        if *count != 1 {
            text.push_str(&count.to_string());
        }
        text.push_str(&format_module_glyph(
            group.beacon,
            group.quality,
            1,
            registry,
        ));
        for ((module, quality), module_count) in &group.modules {
            text.push_str(&format_module_glyph(
                *module,
                *quality,
                *module_count,
                registry,
            ));
        }
    }
    text
}

fn format_module_glyph(
    entry: EntryId,
    quality: Quality,
    count: u32,
    registry: &Registry,
) -> String {
    let glyph = registry.entry_by_index(entry).glyph;
    let quality = match quality {
        Quality::Normal => String::new(),
        quality => quality.glyph().to_string(),
    };
    format!("{quality}{glyph}").repeat(count as usize)
}

fn format_quantity(quantity: &Quantity, settings: &Settings, registry: &Registry) -> String {
    if quantity.second == -1
        && quantity.per.is_none()
        && quantity.joule == 0
        && quantity.craft == 0
        && let Amount::Resources(terms) = &quantity.amount
        && terms.len() == 1
        && let Some((resource, coefficient)) = terms.iter().next()
        && let Some(text) = format_belt_rate(*coefficient, *resource, settings, registry)
    {
        return text;
    }
    if quantity.per.is_none()
        && quantity.craft == 0
        && let Amount::Number(value) = quantity.amount
    {
        match (quantity.joule, quantity.second) {
            (1, 0) => return format_si(value, "J", settings),
            (1, -1) => return format_si(value, "W", settings),
            _ => {}
        }
    }

    // Time counts in the configured unit wherever it appears (§7.4).
    let (amount, seconds_name) = if quantity.second == 0 {
        (quantity.amount.clone(), "s")
    } else {
        let per_unit = settings.time_unit.seconds();
        let factor = per_unit.powi(-quantity.second);
        (
            quantity.amount.scale(factor),
            time_unit_name(settings.time_unit),
        )
    };

    let mut suffix = String::new();
    if let Some(per) = quantity.per {
        suffix.push('/');
        suffix.push_str(&format_resource(per, registry));
    }
    suffix.push_str(&unit_suffix(quantity.joule, "J"));
    suffix.push_str(&unit_suffix(quantity.craft, "craft"));
    suffix.push_str(&unit_suffix(quantity.second, seconds_name));

    match &amount {
        Amount::Number(value) => format!("{}{suffix}", format_number_with(*value, settings)),
        // Every term repeats the units, so the rendering parses back to the
        // same quantity: `1:iron-plate:/s + 2:copper-plate:/s` (§7.5).
        Amount::Resources(terms) => terms
            .iter()
            .map(|(resource, coefficient)| {
                format!(
                    "{}{}{suffix}",
                    format_prefixed(*coefficient, settings, settings.metric_prefixes),
                    format_resource(*resource, registry)
                )
            })
            .collect::<Vec<_>>()
            .join(" + "),
    }
}

fn format_stacks(stacks: &Stacks, rate: bool, settings: &Settings) -> String {
    let fluid = fluid_glyph();
    let (factor, suffix) = if rate {
        (
            settings.time_unit.seconds(),
            format!("/{}", time_unit_name(settings.time_unit)),
        )
    } else {
        (1.0, String::new())
    };

    match stacks {
        Stacks::Items(terms) if terms.is_empty() => {
            format!("0{STACK_SIZE_GLYPH}{suffix}")
        }
        Stacks::Items(terms) => terms
            .iter()
            .map(|(limit, amount)| {
                let subscript = match limit {
                    super::value::StackLimit::Full => String::new(),
                    super::value::StackLimit::Limit(limit) => subscript(*limit),
                };
                format!(
                    "{}{STACK_SIZE_GLYPH}{subscript}{suffix}",
                    format_prefixed(amount * factor, settings, settings.metric_prefixes)
                )
            })
            .collect::<Vec<_>>()
            .join(" + "),
        Stacks::Fluid(amount) => format!(
            "{}{fluid}{suffix}",
            format_prefixed(amount * factor, settings, settings.metric_prefixes)
        ),
    }
}

fn format_resource(resource: Resource, registry: &Registry) -> String {
    let glyph = registry.entry_by_index(resource.id()).glyph;
    match resource.quality() {
        Some(quality) if quality != Quality::Normal => format!("{}{glyph}", quality.glyph()),
        _ => glyph.to_string(),
    }
}

fn format_resource_ref(resource: &ResourceRef, registry: &Registry) -> String {
    let glyph = registry.entry_by_index(resource.id).glyph;
    match resource.quality {
        Some(quality) => format!("{}{glyph}", quality.glyph()),
        None => glyph.to_string(),
    }
}

fn recipe_glyph(registry: &Registry, id: hactorio_data::RecipeId) -> Option<char> {
    registry
        .entries()
        .iter()
        .find(|entry| entry.recipe == Some(id))
        .map(|entry| entry.glyph)
}

/// The fluid signal's glyph, which marks a fluid stack rate.
fn fluid_glyph() -> char {
    hactorio_data::signal_by_id("fluid").map_or('\u{FFFD}', |signal| signal.glyph)
}

fn time_unit_name(unit: TimeUnit) -> &'static str {
    match unit {
        TimeUnit::Second => "s",
        TimeUnit::Minute => "m",
        TimeUnit::Hour => "h",
    }
}

fn unit_suffix(exponent: i32, name: &str) -> String {
    match exponent {
        0 => String::new(),
        1 => name.to_string(),
        -1 => format!("/{name}"),
        exponent if exponent > 1 => format!("{name}{}", superscript(exponent)),
        exponent => format!("/{name}{}", superscript(-exponent)),
    }
}

fn superscript(value: i32) -> String {
    const DIGITS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    digits(value, &DIGITS)
}

fn subscript(value: u32) -> String {
    const DIGITS: [char; 10] = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];
    digits(i32::try_from(value).unwrap_or(0), &DIGITS)
}

fn digits(value: i32, table: &[char; 10]) -> String {
    value
        .to_string()
        .chars()
        .map(|digit| table[digit.to_digit(10).unwrap_or(0) as usize])
        .collect()
}

/// Positional, trailing zeros trimmed, never exponent notation (§7.2).
pub fn format_number(value: f64) -> String {
    format_number_with(value, &Settings::default())
}

fn format_number_with(value: f64, settings: &Settings) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    if super::value::is_zero(value) {
        return "0".to_string();
    }

    let precision = i32::try_from(settings.max_decimals).unwrap_or(3);
    let magnitude = value.abs();
    let decimals = if magnitude >= 1.0 {
        precision
    } else {
        let exponent = magnitude.log10().floor() as i32;
        precision - 1 - exponent
    };
    let decimals = decimals.clamp(0, 12) as usize;

    let mut text = format!("{value:.decimals$}");
    if text.contains('.') {
        text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if text == "-0" { "0".to_string() } else { text }
}

/// `k`/`M`/`G`/`T`, which are also scale suffixes so a rendering round-trips.
fn format_prefixed(value: f64, settings: &Settings, enabled: bool) -> String {
    if !enabled {
        return format_number_with(value, settings);
    }
    let magnitude = value.abs();
    let (scale, prefix) = if magnitude >= 1e12 {
        (1e12, "T")
    } else if magnitude >= 1e9 {
        (1e9, "G")
    } else if magnitude >= 1e6 {
        (1e6, "M")
    } else if magnitude >= 1e3 {
        (1e3, "k")
    } else {
        (1.0, "")
    };
    format!("{}{prefix}", format_number_with(value / scale, settings))
}

/// SI prefixes for joules and watts (§7.3).
fn format_si(value: f64, unit: &str, settings: &Settings) -> String {
    if value.is_nan() || value.is_infinite() {
        return format!("{}{unit}", format_number_with(value, settings));
    }
    format!("{}{unit}", format_prefixed(value, settings, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_eval::test_registry;

    fn settings() -> Settings {
        Settings::default()
    }

    #[test]
    fn numbers_use_fraction_digits_above_one_and_significant_digits_below() {
        assert_eq!(format_number(123_456.789_1), "123456.789");
        assert_eq!(format_number(1.0 / 3.0), "0.333");
        assert_eq!(format_number(0.000_123_4), "0.000123");
        assert_eq!(format_number(2.0), "2");
        assert_eq!(format_number(0.0), "0");
        assert_eq!(format_number(-0.0), "0");
        assert_eq!(format_number(1e-12), "0");
    }

    #[test]
    fn quantities_render_round_trippably() {
        let registry = test_registry();
        let iron = registry.entry_id("iron-plate").unwrap();
        let quantity = Quantity::resource(Resource::Item(super::super::value::Item {
            id: iron,
            quality: Quality::Normal,
        }));
        assert_eq!(
            format_quantity(&quantity, &settings(), &registry),
            format!("1{}", registry.entry_by_index(iron).glyph)
        );
    }

    #[test]
    fn energy_and_power_use_si_prefixes() {
        let registry = test_registry();
        let energy = Quantity::new(Amount::Number(1500.0), None, 0, 1, 0);
        assert_eq!(format_quantity(&energy, &settings(), &registry), "1.5kJ");
        let power = Quantity::new(Amount::Number(2e6), None, -1, 1, 0);
        assert_eq!(format_quantity(&power, &settings(), &registry), "2MW");
    }

    #[test]
    fn time_follows_the_configured_unit() {
        let registry = test_registry();
        let mut minute = settings();
        minute.time_unit = TimeUnit::Minute;

        let duration = Quantity::time(180.0);
        assert_eq!(format_quantity(&duration, &settings(), &registry), "180s");
        assert_eq!(format_quantity(&duration, &minute, &registry), "3m");

        let rate = Quantity::new(Amount::Number(15.0), None, -1, 0, 0);
        assert_eq!(format_quantity(&rate, &settings(), &registry), "15/s");
        assert_eq!(format_quantity(&rate, &minute, &registry), "900/m");
    }

    #[test]
    fn stack_rates_print_a_subscript_for_a_limited_cap() {
        use super::super::value::StackLimit;
        let stacks = Stacks::Items(std::collections::BTreeMap::from([
            (StackLimit::Limit(1), 5.01),
            (StackLimit::Full, 3.2),
        ]));
        let text = format_stacks(&stacks, true, &settings());
        assert_eq!(
            text,
            format!("3.2{STACK_SIZE_GLYPH}/s + 5.01{STACK_SIZE_GLYPH}₁/s")
        );
    }

    #[test]
    fn metric_prefixes_are_opt_in() {
        let mut off = settings();
        off.metric_prefixes = false;
        assert_eq!(format_prefixed(1500.0, &settings(), true), "1.5k");
        assert_eq!(format_prefixed(1500.0, &off, off.metric_prefixes), "1500");
    }
}
