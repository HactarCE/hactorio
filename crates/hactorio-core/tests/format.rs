use hactorio_core::{BaseUnit, BeltTier, Settings, TimeUnit, Value, format_value};
use hactorio_data::{Locale, Registry, postprocess};

const DUMP: &str = r#"{
  "item": {
    "iron-plate": {
      "type": "item",
      "name": "iron-plate",
      "stack_size": 100,
      "icon": "__base__/graphics/icons/iron-plate.png"
    }
  },
  "fluid": {
    "water": {
      "type": "fluid",
      "name": "water",
      "icon": "__base__/graphics/icons/fluid/water.png"
    }
  }
}"#;

fn registry() -> Registry {
    postprocess(DUMP, &Locale::empty()).unwrap()
}

fn plate(registry: &Registry) -> BaseUnit {
    BaseUnit::Item(registry.entry_id("iron-plate").unwrap())
}

fn water(registry: &Registry) -> BaseUnit {
    BaseUnit::Fluid(registry.entry_id("water").unwrap())
}

fn glyph(registry: &Registry, id: &str) -> String {
    registry.entry(id).unwrap().glyph.to_string()
}

fn item_rate(registry: &Registry, coef: f64) -> Value {
    Value::new(coef, [(plate(registry), 1), (BaseUnit::Second, -1)])
}

#[test]
fn scalars_use_the_decimal_limit_and_trim_zeros() {
    let registry = registry();
    let settings = Settings::default();

    assert_eq!(
        format_value(&Value::scalar(1.0 / 3.0), &settings, &registry),
        "0.333"
    );
    assert_eq!(
        format_value(&Value::scalar(2.5), &settings, &registry),
        "2.5"
    );
    assert_eq!(format_value(&Value::scalar(2.0), &settings, &registry), "2");
    assert_eq!(format_value(&Value::scalar(0.0), &settings, &registry), "0");

    let terse = Settings {
        max_decimals: 1,
        ..Settings::default()
    };
    assert_eq!(
        format_value(&Value::scalar(1.0 / 3.0), &terse, &registry),
        "0.3"
    );
}

#[test]
fn item_quantities_use_metric_prefixes_when_enabled() {
    let registry = registry();
    let value = Value::new(3000.0, [(plate(&registry), 1)]);
    let glyph = glyph(&registry, "iron-plate");

    let prefixed = Settings::default();
    assert_eq!(
        format_value(&value, &prefixed, &registry),
        format!("3k {glyph}")
    );

    let plain = Settings {
        metric_prefixes: false,
        ..Settings::default()
    };
    assert_eq!(
        format_value(&value, &plain, &registry),
        format!("3000 {glyph}")
    );

    let negative = Value::new(-3000.0, [(plate(&registry), 1)]);
    assert_eq!(
        format_value(&negative, &prefixed, &registry),
        format!("-3k {glyph}")
    );
}

#[test]
fn time_unit_scales_the_displayed_coefficient() {
    let registry = registry();
    let value = item_rate(&registry, 3.0);
    let glyph = glyph(&registry, "iron-plate");

    let minute = Settings {
        time_unit: TimeUnit::Minute,
        ..Settings::default()
    };
    assert_eq!(
        format_value(&value, &minute, &registry),
        format!("180 {glyph}/m")
    );

    let hour = Settings {
        time_unit: TimeUnit::Hour,
        ..Settings::default()
    };
    assert_eq!(
        format_value(&Value::seconds(3600.0), &hour, &registry),
        "1 h"
    );
}

#[test]
fn craft_rates_print_as_craft_s() {
    let registry = registry();
    let value = Value::new(2.0, [(BaseUnit::Craft, 1), (BaseUnit::Second, -1)]);
    let minute = Settings {
        time_unit: TimeUnit::Minute,
        ..Settings::default()
    };
    assert_eq!(format_value(&value, &minute, &registry), "2 craft/s");
    assert_eq!(
        format_value(&Value::crafts(3.0), &minute, &registry),
        "3 craft"
    );
}

#[test]
fn numerator_and_denominator_are_rendered_explicitly() {
    let registry = registry();
    let settings = Settings::default();
    let glyph = glyph(&registry, "iron-plate");

    let rate = Value::new(2.0, [(BaseUnit::Second, -1)]);
    assert_eq!(format_value(&rate, &settings, &registry), "2/s");

    let inverse_square = Value::new(4.0, [(plate(&registry), -2)]);
    assert_eq!(
        format_value(&inverse_square, &settings, &registry),
        format!("4/{glyph}^2")
    );

    let combined = Value::new(
        2.0,
        [
            (plate(&registry), 1),
            (BaseUnit::Second, -1),
            (BaseUnit::Joule, -1),
        ],
    );
    assert_eq!(
        format_value(&combined, &settings, &registry),
        format!("2 {glyph}/(joule * s)")
    );
}

#[test]
fn electricity_always_uses_metric_prefixes() {
    let registry = registry();
    let plain_items = Settings {
        metric_prefixes: false,
        ..Settings::default()
    };
    assert_eq!(
        format_value(&Value::watts(1_500_000.0), &plain_items, &registry),
        "1.5M joule/s"
    );
}

#[test]
fn belt_modes_show_item_rates_as_belt_counts() {
    let registry = registry();
    let glyph = glyph(&registry, "iron-plate");
    let belts = Settings {
        item_rate_belt: Some(BeltTier::Green),
        ..Settings::default()
    };

    // One green belt carries 60 items per second.
    assert_eq!(
        format_value(&item_rate(&registry, 960.0), &belts, &registry),
        format!("16 gb {glyph}")
    );
    assert_eq!(
        format_value(&item_rate(&registry, 100.0), &belts, &registry),
        format!("1.667 gb {glyph}")
    );

    // Stacked green belts carry 60 * 4 = 240 items per second.
    let stacked = Settings {
        item_rate_belt: Some(BeltTier::Green),
        stacked_belts: true,
        ..Settings::default()
    };
    assert_eq!(
        format_value(&item_rate(&registry, 960.0), &stacked, &registry),
        format!("4 sgb {glyph}")
    );
}

#[test]
fn belt_modes_ignore_fluids_and_composite_units() {
    let registry = registry();
    let belts = Settings {
        item_rate_belt: Some(BeltTier::Green),
        ..Settings::default()
    };

    let fluid_rate = Value::new(3.0, [(water(&registry), 1), (BaseUnit::Second, -1)]);
    assert_eq!(
        format_value(&fluid_rate, &belts, &registry),
        format!("3 {}/s", glyph(&registry, "water"))
    );

    let composite = Value::new(3.0, [(plate(&registry), 1), (BaseUnit::Second, -2)]);
    assert_eq!(
        format_value(&composite, &belts, &registry),
        format!("3 {}/s^2", glyph(&registry, "iron-plate"))
    );
}

#[test]
fn non_finite_values_are_spelled_out() {
    let registry = registry();
    let settings = Settings::default();

    assert_eq!(
        format_value(&Value::scalar(f64::INFINITY), &settings, &registry),
        "inf"
    );
    assert_eq!(
        format_value(&Value::scalar(f64::NEG_INFINITY), &settings, &registry),
        "-inf"
    );
    assert_eq!(
        format_value(&Value::scalar(f64::NAN), &settings, &registry),
        "NaN"
    );
}

#[test]
fn zero_coefficients_keep_their_units() {
    let registry = registry();
    let settings = Settings::default();
    let value = Value::new(0.0, [(plate(&registry), 1)]);
    assert_eq!(
        format_value(&value, &settings, &registry),
        format!("0 {}", glyph(&registry, "iron-plate"))
    );
}
