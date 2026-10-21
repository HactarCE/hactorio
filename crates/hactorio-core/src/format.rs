//! Formatting of scalar values.

use hactorio_data::{EntryId, Registry};

use crate::belt::belt_capacity;
use crate::eval::{Output, RecipeOutput, RecipeQuantity};
use crate::settings::{Settings, TimeUnit};
use crate::value::{BaseUnit, Value};

/// How the coefficient is scaled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberStyle {
    Plain,
    Metric,
}

/// Formats a value for the output gutter.
///
/// Prototype units are rendered as their private-use glyph, so the app's glyph
/// rasterizer draws the icon. Time follows the global time unit, item
/// quantities follow the prefix toggle (belt modes take over item rates), and
/// electricity always uses metric prefixes.
pub fn format_value(value: &Value, settings: &Settings, registry: &Registry) -> String {
    if value.is_dimensionless() {
        return format_number(value.coef(), settings, NumberStyle::Plain);
    }

    if is_craft_rate(value) {
        return format!(
            "{} craft/s",
            format_number(value.coef(), settings, NumberStyle::Plain)
        );
    }

    if let Some(belts) = format_belt_rate(value, settings, registry) {
        return belts;
    }

    let (coefficient, unit_name) = scaled_time(value, settings);
    let number = format_number(coefficient, settings, number_style(value, settings));

    let mut parts: Vec<(String, i64)> = value
        .units()
        .iter()
        .map(|(&unit, &exponent)| (unit_name_of(unit, unit_name, registry), exponent))
        .collect();
    parts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let numerator: Vec<String> = parts
        .iter()
        .filter(|(_, exponent)| *exponent > 0)
        .map(|(name, exponent)| render_unit(name, *exponent))
        .collect();
    let denominator: Vec<String> = parts
        .iter()
        .filter(|(_, exponent)| *exponent < 0)
        .map(|(name, exponent)| render_unit(name, -*exponent))
        .collect();

    if numerator.is_empty() {
        return format!("{number}/{}", join_units(&denominator));
    }
    let mut text = format!("{number} {}", numerator.join(" * "));
    if !denominator.is_empty() {
        text.push('/');
        text.push_str(&join_units(&denominator));
    }
    text
}

fn is_craft_rate(value: &Value) -> bool {
    value.units().len() == 2
        && value.exponent(BaseUnit::Craft) == 1
        && value.exponent(BaseUnit::Second) == -1
}

/// Item rates in a belt mode are shown as belt counts; fluids ignore the mode.
fn format_belt_rate(value: &Value, settings: &Settings, registry: &Registry) -> Option<String> {
    let tier = settings.item_rate_belt?;
    let token = if settings.stacked_belts {
        tier.stacked_token()
    } else {
        tier.token()
    };
    if value.units().len() != 2 || value.exponent(BaseUnit::Second) != -1 {
        return None;
    }
    let mut item = None;
    for &unit in value.units().keys() {
        match unit {
            BaseUnit::Item(id) if item.is_none() => item = Some(id),
            BaseUnit::Second => {}
            _ => return None,
        }
    }
    let item = item?;
    if value.exponent(BaseUnit::Item(item)) != 1 {
        return None;
    }
    let stack_size = item_stack_size(registry, item)?;
    let capacity = belt_capacity(
        tier,
        settings.stacked_belts,
        stack_size,
        settings.belt_stacking,
    );
    let belts = value.coef() / capacity;
    Some(format!(
        "{} {token} {}",
        format_number(belts, settings, NumberStyle::Plain),
        registry.entry_by_index(item).glyph
    ))
}

fn scaled_time(value: &Value, settings: &Settings) -> (f64, TimeUnit) {
    let exponent = value.exponent(BaseUnit::Second);
    if exponent == 0 || settings.time_unit == TimeUnit::Second {
        return (value.coef(), settings.time_unit);
    }
    let scale = settings.time_unit.seconds().powf(-(exponent as f64));
    (value.coef() * scale, settings.time_unit)
}

fn number_style(value: &Value, settings: &Settings) -> NumberStyle {
    let has_joule = value.units().keys().any(|unit| *unit == BaseUnit::Joule);
    let has_item = value
        .units()
        .keys()
        .any(|unit| matches!(unit, BaseUnit::Item(_)));
    if has_joule || (has_item && settings.metric_prefixes) {
        NumberStyle::Metric
    } else {
        NumberStyle::Plain
    }
}

fn unit_name_of(unit: BaseUnit, time_unit: TimeUnit, registry: &Registry) -> String {
    match unit {
        BaseUnit::Item(id) | BaseUnit::Fluid(id) => registry.entry_by_index(id).glyph.to_string(),
        BaseUnit::Second => time_unit.short_name().to_string(),
        BaseUnit::Joule => "joule".to_string(),
        BaseUnit::Craft => "craft".to_string(),
    }
}

fn render_unit(name: &str, exponent: i64) -> String {
    if exponent == 1 {
        name.to_string()
    } else {
        format!("{name}^{exponent}")
    }
}

fn join_units(units: &[String]) -> String {
    if units.len() == 1 {
        units[0].clone()
    } else {
        format!("({})", units.join(" * "))
    }
}

fn item_stack_size(registry: &Registry, item: EntryId) -> Option<u32> {
    registry
        .entry_by_index(item)
        .item
        .as_ref()
        .map(|item| item.stack_size)
}

fn format_number(value: f64, settings: &Settings, style: NumberStyle) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    let (scaled, suffix) = match style {
        NumberStyle::Plain => (value, ""),
        NumberStyle::Metric => metric_scale(value),
    };
    let mut text = format_decimal(scaled, settings.max_decimals);
    text.push_str(suffix);
    text
}

fn metric_scale(value: f64) -> (f64, &'static str) {
    let magnitude = value.abs();
    if magnitude >= 1e9 {
        (value / 1e9, "G")
    } else if magnitude >= 1e6 {
        (value / 1e6, "M")
    } else if magnitude >= 1e3 {
        (value / 1e3, "k")
    } else {
        (value, "")
    }
}

fn format_decimal(value: f64, max_decimals: usize) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let mut text = format!("{value:.max_decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

/// A rendered gutter output: one headline and optional detail rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Formatted {
    pub headline: String,
    pub rows: Vec<String>,
}

/// Renders the gutter output for one statement.
pub fn format_output(output: &Output, settings: &Settings, registry: &Registry) -> Formatted {
    match output {
        Output::Scalar(value) => Formatted {
            headline: format_value(value, settings, registry),
            rows: Vec::new(),
        },
        Output::Machine(machine) => Formatted {
            headline: machine_headline(machine.machine, machine.count, settings, registry),
            rows: Vec::new(),
        },
        Output::Recipe(recipe) => format_recipe(recipe, settings, registry),
    }
}

fn format_recipe(
    recipe: &RecipeOutput,
    settings: &Settings,
    registry: &Registry,
) -> Formatted {
    let rate = matches!(recipe.quantity, RecipeQuantity::Machines(_));
    let machine_glyph = registry.entry_by_index(recipe.machine).glyph;
    let headline = if rate {
        machine_headline(
            recipe.machine,
            recipe.quantity.magnitude(),
            settings,
            registry,
        )
    } else {
        let unit = settings.time_unit;
        let magnitude = recipe.quantity.magnitude() / unit.seconds();
        format!(
            "{} {machine_glyph}·{}",
            format_plain_number(magnitude, settings),
            unit.short_name()
        )
    };

    let mut rows = Vec::new();
    let mut units: Vec<(BaseUnit, f64)> = recipe
        .net
        .iter()
        .filter(|(unit, _)| **unit != BaseUnit::Craft)
        .map(|(unit, amount)| (*unit, *amount))
        .collect();
    units.sort_by_key(|(unit, _)| unit_name(*unit, registry));

    for (unit, amount) in units {
        let sign = if amount > 0.0 { "+" } else { "-" };
        rows.push(format!(
            "{sign}{}",
            format_amount(amount.abs(), unit, rate, settings, registry)
        ));
    }

    if let Some(craft) = recipe.net.get(&BaseUnit::Craft).copied() {
        let value = if rate {
            Value::new(craft, [(BaseUnit::Craft, 1), (BaseUnit::Second, -1)])
        } else {
            Value::crafts(craft)
        };
        rows.push(format_value(&value, settings, registry));
    }

    if rate {
        rows.push(format!(
            "{}/{machine_glyph}",
            format_power(recipe.per_machine_power_w, settings)
        ));
        let count = recipe.quantity.magnitude();
        if count != 1.0 {
            rows.push(format!(
                "{} total",
                format_power(recipe.per_machine_power_w * count, settings)
            ));
        }
    } else if let Some(energy) = recipe.total_energy_j {
        rows.push(format!("{} total energy", format_energy(energy, settings)));
    }

    Formatted { headline, rows }
}

fn machine_headline(
    machine: EntryId,
    count: f64,
    settings: &Settings,
    registry: &Registry,
) -> String {
    format!(
        "{} {}",
        format_value(&Value::scalar(count), settings, registry),
        registry.entry_by_index(machine).glyph
    )
}

/// Formats one unit amount, as a rate or a total.
pub fn format_amount(
    amount: f64,
    unit: BaseUnit,
    rate: bool,
    settings: &Settings,
    registry: &Registry,
) -> String {
    let value = if rate {
        Value::new(amount, [(unit, 1), (BaseUnit::Second, -1)])
    } else {
        Value::new(amount, [(unit, 1)])
    };
    format_value(&value, settings, registry)
}

/// Formats watts with a metric prefix.
pub fn format_power(watts: f64, settings: &Settings) -> String {
    format_si(watts, "W", settings)
}

/// Formats joules with a metric prefix.
pub fn format_energy(joules: f64, settings: &Settings) -> String {
    format_si(joules, "J", settings)
}

pub(crate) fn format_plain_number(value: f64, settings: &Settings) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    format_decimal(value, settings.max_decimals)
}

fn format_si(value: f64, unit: &str, settings: &Settings) -> String {
    if value.is_nan() {
        return format!("NaN {unit}");
    }
    if value.is_infinite() {
        return format!("{} {unit}", if value < 0.0 { "-inf" } else { "inf" });
    }
    let (scaled, prefix) = metric_scale(value);
    format!(
        "{} {prefix}{unit}",
        format_decimal(scaled, settings.max_decimals)
    )
}

fn unit_name(unit: BaseUnit, registry: &Registry) -> String {
    match unit {
        BaseUnit::Item(id) | BaseUnit::Fluid(id) => {
            registry.entry_by_index(id).id.to_string()
        }
        BaseUnit::Second => "second".to_string(),
        BaseUnit::Joule => "joule".to_string(),
        BaseUnit::Craft => "craft".to_string(),
    }
}
