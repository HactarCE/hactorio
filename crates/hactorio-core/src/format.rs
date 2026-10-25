//! Formatting of scalar values.

use hactorio_data::{EntryId, Registry};

use crate::belt::belt_capacity;
use crate::settings::{Settings, TimeUnit};
use crate::value::{BaseUnit, Value, is_zero};

/// How the coefficient is scaled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumberStyle {
    Plain,
    Metric,
}

/// Formats a value for a single-line result.
///
/// Prototype units are rendered as their private-use glyph, so the app's glyph
/// rasterizer draws the icon. Time follows the global time unit, item
/// quantities follow the prefix toggle (belt modes take over item rates), and
/// electricity is rendered as `J`/`W` with metric prefixes.
pub fn format_value(value: &Value, settings: &Settings, registry: &Registry) -> String {
    if value.is_dimensionless() {
        return format_number(value.coef(), settings, NumberStyle::Plain);
    }

    if is_energy(value) {
        return format_energy(value.coef(), settings);
    }
    if is_power(value) {
        return format_power(value.coef(), settings);
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

/// Exactly one joule, with no other units.
fn is_energy(value: &Value) -> bool {
    value.units().len() == 1 && value.exponent(BaseUnit::Joule) == 1
}

/// Exactly one joule per second. Power is shown in watts regardless of the
/// global time unit, since a watt does not depend on a display timescale.
fn is_power(value: &Value) -> bool {
    value.units().len() == 2
        && value.exponent(BaseUnit::Joule) == 1
        && value.exponent(BaseUnit::Second) == -1
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
    // The tier's belt icon replaces `yb`/`rb`/…; a stacked rate puts the
    // stack-size signal in front of it.
    let belt = match registry.entry(tier.item_id()) {
        Some(entry) if settings.stacked_belts => {
            format!("{}{}", hactorio_data::STACK_SIZE_GLYPH, entry.glyph)
        }
        Some(entry) => entry.glyph.to_string(),
        None => token.to_string(),
    };
    Some(format!(
        "{} {belt} {}",
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
        BaseUnit::Joule => "J".to_string(),
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
    if is_zero(value) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use hactorio_data::{Locale, postprocess};

    fn registry() -> Registry {
        postprocess("{}", &Locale::empty()).unwrap()
    }

    fn energy(joules: f64) -> Value {
        Value::new(joules, [(BaseUnit::Joule, 1)])
    }

    fn power(watts: f64) -> Value {
        Value::new(watts, [(BaseUnit::Joule, 1), (BaseUnit::Second, -1)])
    }

    #[test]
    fn energy_uses_joule_symbol() {
        let registry = registry();
        let settings = Settings::default();
        for (joules, expected) in [
            (0.0, "0 J"),
            (3.0, "3 J"),
            (3e3, "3 kJ"),
            (3e6, "3 MJ"),
            (3e9, "3 GJ"),
            (10.8e9, "10.8 GJ"),
        ] {
            assert_eq!(
                format_value(&energy(joules), &settings, &registry),
                expected
            );
        }
    }

    #[test]
    fn power_uses_watt_symbol() {
        let registry = registry();
        let settings = Settings::default();
        for (watts, expected) in [
            (3.0, "3 W"),
            (375e3, "375 kW"),
            (2.5e6, "2.5 MW"),
            (0.0, "0 W"),
        ] {
            assert_eq!(format_value(&power(watts), &settings, &registry), expected);
        }
    }

    #[test]
    fn power_ignores_time_unit() {
        let registry = registry();
        for time_unit in [TimeUnit::Minute, TimeUnit::Hour] {
            let settings = Settings {
                time_unit,
                ..Settings::default()
            };
            assert_eq!(format_value(&power(375e3), &settings, &registry), "375 kW");
        }
    }

    #[test]
    fn compound_joule_units_keep_numeric_prefix() {
        let registry = registry();
        let settings = Settings::default();
        let per_second_squared = Value::new(3e3, [(BaseUnit::Joule, 1), (BaseUnit::Second, -2)]);
        assert_eq!(
            format_value(&per_second_squared, &settings, &registry),
            "3k J/s^2"
        );
        let joule_seconds = Value::new(3.0, [(BaseUnit::Joule, 1), (BaseUnit::Second, 1)]);
        assert_eq!(
            format_value(&joule_seconds, &settings, &registry),
            "3 J * s"
        );
    }
}
