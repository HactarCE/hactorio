use hactorio_core::{
    BaseUnit, BeltTier, Settings, TimeUnit, Value, ValueError, belt_capacity, belt_value,
    stack_size,
};

fn item() -> BaseUnit {
    BaseUnit::Item(hactorio_data::EntryId::from_index(0))
}

#[test]
fn zero_exponents_are_canonicalized() {
    let value = Value::new(2.0, [(BaseUnit::Second, 0)]);
    assert!(value.is_dimensionless());
    assert_eq!(value.units().len(), 0);
}

#[test]
fn aliases_fold_into_base_units() {
    assert_eq!(
        Value::minutes(3.0),
        Value::new(180.0, [(BaseUnit::Second, 1)])
    );
    assert_eq!(
        Value::hours(1.0),
        Value::new(3600.0, [(BaseUnit::Second, 1)])
    );
    assert_eq!(
        Value::watts(2.0),
        Value::new(2.0, [(BaseUnit::Joule, 1), (BaseUnit::Second, -1)])
    );
    assert_eq!(Value::crafts(3.0), Value::new(3.0, [(BaseUnit::Craft, 1)]));
}

#[test]
fn multiplication_adds_exponents_and_division_subtracts_them() {
    let rate = Value::new(3.0, [(item(), 1)]).div(&Value::seconds(1.0));
    assert_eq!(rate.exponent(item()), 1);
    assert_eq!(rate.exponent(BaseUnit::Second), -1);
    assert_eq!(rate.coef(), 3.0);

    let quantity = rate.mul(&Value::minutes(2.0));
    assert_eq!(quantity.exponent(BaseUnit::Second), 0);
    assert_eq!(quantity.coef(), 360.0);
}

#[test]
fn division_by_zero_is_not_an_error() {
    let value = Value::scalar(1.0).div(&Value::scalar(0.0));
    assert!(value.coef().is_infinite());
}

#[test]
fn addition_requires_identical_units() {
    let plates = Value::new(1.0, [(item(), 1)]);
    let seconds = Value::seconds(1.0);

    assert_eq!(plates.add(&seconds), Err(ValueError::IncompatibleUnits));
    assert_eq!(
        plates.add(&plates).unwrap().coef(),
        2.0,
        "same units add coefficients"
    );
    assert_eq!(plates.sub(&plates).unwrap().coef(), 0.0);
}

#[test]
fn powers_of_dimensionless_values_are_real() {
    let root = Value::scalar(4.0).pow(&Value::scalar(0.5)).unwrap();
    assert_eq!(root.coef(), 2.0);

    let inverse = Value::scalar(2.0).pow(&Value::scalar(-2.0)).unwrap();
    assert_eq!(inverse.coef(), 0.25);
}

#[test]
fn powers_of_values_with_units_require_integers() {
    let plate = Value::new(1.0, [(item(), 1)]);

    let square = plate.pow(&Value::scalar(2.0)).unwrap();
    assert_eq!(square.exponent(item()), 2);

    let inverse = plate.pow(&Value::scalar(-1.0)).unwrap();
    assert_eq!(inverse.exponent(item()), -1);

    assert_eq!(
        plate.pow(&Value::scalar(0.5)),
        Err(ValueError::FractionalExponent)
    );
    assert_eq!(
        Value::scalar(2.0).pow(&Value::seconds(1.0)),
        Err(ValueError::DimensionalExponent)
    );
    assert_eq!(
        plate.pow(&Value::scalar(1e30)),
        Err(ValueError::ExponentOverflow)
    );
}

#[test]
fn negation_keeps_units() {
    let value = Value::new(2.0, [(item(), 1)]).neg();
    assert_eq!(value.coef(), -2.0);
    assert_eq!(value.exponent(item()), 1);
}

#[test]
fn time_unit_words_include_short_aliases() {
    use TimeUnit::{Hour, Minute, Second};

    for (word, unit) in [
        ("second", Second),
        ("s", Second),
        ("minute", Minute),
        ("m", Minute),
        ("hour", Hour),
        ("h", Hour),
    ] {
        assert_eq!(TimeUnit::from_word(word), Some(unit), "{word}");
    }

    assert_eq!(TimeUnit::from_word("hours"), None);
    assert_eq!(TimeUnit::from_word("M"), None, "M is a numeric suffix");
    assert_eq!(TimeUnit::from_word(""), None);

    assert_eq!(TimeUnit::Minute.value(3.0), Value::minutes(3.0));
    assert_eq!(TimeUnit::Hour.value(2.0), Value::hours(2.0));
    assert_eq!(TimeUnit::Second.value(5.0), Value::seconds(5.0));
}

#[test]
fn belt_stacking_is_the_smaller_of_level_and_stack_size() {
    assert_eq!(stack_size(4, 100), 4);
    assert_eq!(stack_size(4, 3), 3);
    assert_eq!(stack_size(0, 100), 1, "level is clamped to 1..=4");
    assert_eq!(stack_size(9, 100), 4);
}

#[test]
fn belt_capacity_scales_with_tier_and_stacking() {
    assert_eq!(belt_capacity(BeltTier::Yellow, false, 100, 4), 15.0);
    assert_eq!(belt_capacity(BeltTier::Green, false, 100, 4), 60.0);
    assert_eq!(belt_capacity(BeltTier::Green, true, 100, 4), 240.0);
    assert_eq!(
        belt_capacity(BeltTier::Green, true, 2, 4),
        120.0,
        "stack size caps the effective stack"
    );
}

#[test]
fn belt_literals_become_item_rates() {
    let value = belt_value(2.0, BeltTier::Green, true, item_id(), 100, 4);
    assert_eq!(value.coef(), 480.0);
    assert_eq!(value.exponent(item()), 1);
    assert_eq!(value.exponent(BaseUnit::Second), -1);
}

#[test]
fn settings_defaults_and_serialization() {
    let settings = Settings::default();
    assert_eq!(settings.time_unit, TimeUnit::Second);
    assert_eq!(settings.item_rate_belt, None);
    assert!(!settings.stacked_belts);
    assert_eq!(settings.max_decimals, 3);
    assert!(settings.metric_prefixes);
    assert_eq!(settings.belt_stacking, 4);

    let json = serde_json::to_string(&settings).unwrap();
    assert!(json.contains("\"time_unit\":\"second\""));
    assert!(json.contains("\"item_rate_belt\":null"));
    assert!(json.contains("\"stacked_belts\":false"));
    assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);

    let belt = Settings {
        item_rate_belt: Some(BeltTier::Green),
        stacked_belts: true,
        ..Settings::default()
    };
    let json = serde_json::to_string(&belt).unwrap();
    assert!(json.contains("\"item_rate_belt\":\"green\""));
    assert!(json.contains("\"stacked_belts\":true"));
    assert_eq!(
        serde_json::from_str::<Settings>("{\"time_unit\":\"minute\"}")
            .unwrap()
            .time_unit,
        TimeUnit::Minute
    );
}

fn item_id() -> hactorio_data::EntryId {
    hactorio_data::EntryId::from_index(0)
}
