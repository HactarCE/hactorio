use hactorio_core::{
    Breakdown, ItemRateDisplay, LineCache, LineOutput, Output, RecipeQuantity, Settings,
    StatementOutput, breakdown, evaluate, format_output,
};
use hactorio_data::{Locale, Registry, postprocess};

const DUMP: &str = include_str!("fixtures/dump.json");

fn registry() -> Registry {
    postprocess(DUMP, &Locale::empty()).unwrap()
}

fn run(registry: &Registry, tab: &str) -> Vec<StatementOutput> {
    evaluate(tab, &Settings::default(), registry, &mut LineCache::new())
}

fn run_with(registry: &Registry, settings: &Settings, tab: &str) -> Vec<StatementOutput> {
    evaluate(tab, settings, registry, &mut LineCache::new())
}

fn output(output: &StatementOutput) -> &Output {
    match &output.output {
        LineOutput::Value(output) => output,
        other => panic!("expected a value, got {other:?}"),
    }
}

fn breakdown_of(output: &StatementOutput, settings: &Settings, registry: &Registry) -> Breakdown {
    breakdown(
        output.eval.as_ref().expect("evaluated value"),
        settings,
        registry,
    )
}

fn section<'a>(breakdown: &'a Breakdown, title: &str) -> &'a hactorio_core::BreakdownSection {
    breakdown
        .sections
        .iter()
        .find(|section| section.title == title)
        .unwrap_or_else(|| panic!("no {title} section in {breakdown:?}"))
}

fn row_value<'a>(breakdown: &'a Breakdown, section_title: &str, label: &str) -> &'a str {
    section(breakdown, section_title)
        .rows
        .iter()
        .find(|row| row.label == label)
        .unwrap_or_else(|| panic!("no row {label} in {}", section_title))
        .value
        .as_str()
}

#[test]
fn scalars_render_as_one_line() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = run(&registry, "3 * 1.5");
    let formatted = format_output(output(&outputs[0]), &settings, &registry);

    assert_eq!(formatted.headline, "4.5");
    assert!(formatted.rows.is_empty());
}

#[test]
fn machine_values_render_count_and_glyph() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = run(&registry, "2 * @:assembling-machine-3:");
    let glyph = registry.entry("assembling-machine-3").unwrap().glyph;

    assert_eq!(
        format_output(output(&outputs[0]), &settings, &registry).headline,
        format!("2 {glyph}")
    );
}

#[test]
fn machine_rates_render_produces_consumes_craft_and_power() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = run(
        &registry,
        "100 :electronic-circuit: / second in @:assembling-machine-3: making :electronic-circuit:",
    );
    let formatted = format_output(output(&outputs[0]), &settings, &registry);
    let circuit = registry.entry("electronic-circuit").unwrap().glyph;
    let plate = registry.entry("iron-plate").unwrap().glyph;

    assert_eq!(formatted.headline, format!("40 {}", registry.entry("assembling-machine-3").unwrap().glyph));
    assert!(
        formatted
            .rows
            .contains(&format!("+100 {circuit}/s")),
        "{:?}",
        formatted.rows
    );
    assert!(
        formatted
            .rows
            .contains(&format!("-100 {plate}/s")),
        "{:?}",
        formatted.rows
    );
    assert!(formatted.rows.contains(&"100 craft/s".to_string()), "{:?}", formatted.rows);
    assert!(
        formatted.rows.contains(&format!("375 kW/{}", registry.entry("assembling-machine-3").unwrap().glyph)),
        "{:?}",
        formatted.rows
    );
    assert!(
        formatted.rows.contains(&"15 MW total".to_string()),
        "{:?}",
        formatted.rows
    );
}

#[test]
fn machine_seconds_render_totals_and_energy() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = run(&registry, "100 :iron-plate: in @:electric-furnace:");
    let formatted = format_output(output(&outputs[0]), &settings, &registry);
    let plate = registry.entry("iron-plate").unwrap().glyph;

    assert_eq!(formatted.headline, format!("160 {}·s", registry.entry("electric-furnace").unwrap().glyph));
    assert!(
        formatted.rows.contains(&format!("+100 {plate}")),
        "{:?}",
        formatted.rows
    );
    assert!(
        formatted.rows.contains(&"100 craft".to_string()),
        "{:?}",
        formatted.rows
    );
    assert!(
        formatted.rows.contains(&"28.8 MJ total energy".to_string()),
        "{:?}",
        formatted.rows
    );
    assert!(!formatted.rows.iter().any(|row| row.contains("per machine")));
}

#[test]
fn belt_mode_applies_to_rate_rows() {
    let registry = registry();
    let settings = Settings {
        item_rate_display: ItemRateDisplay::Gb,
        ..Settings::default()
    };
    let outputs = run_with(
        &registry,
        &settings,
        "60 :iron-plate: / second in @:electric-furnace:",
    );
    let formatted = format_output(output(&outputs[0]), &settings, &registry);
    let plate = registry.entry("iron-plate").unwrap().glyph;

    assert!(
        formatted.rows.contains(&format!("+1 gb {plate}")),
        "{:?}",
        formatted.rows
    );
}

#[test]
fn infinity_renders_in_the_header() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = run(
        &registry,
        ":catalyst: / second in @:assembling-machine-3:",
    );
    let formatted = format_output(output(&outputs[0]), &settings, &registry);
    let glyph = registry.entry("assembling-machine-3").unwrap().glyph;

    assert_eq!(formatted.headline, format!("inf {glyph}"));
    assert!(matches!(
        output(&outputs[0]),
        Output::Recipe(recipe)
            if matches!(recipe.quantity, RecipeQuantity::Machines(value) if value.is_infinite())
    ));
}

#[test]
fn breakdown_covers_configuration_recipe_items_power_and_pollution() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = run(
        &registry,
        "@:assembling-machine-3: 2 :productivity-module-3: making :electronic-circuit:",
    );
    let breakdown = breakdown_of(&outputs[0], &settings, &registry);

    assert_eq!(row_value(&breakdown, "machine", "slots"), "2 / 4");
    assert_eq!(
        row_value(&breakdown, "machine", "productivity"),
        "20%"
    );
    assert_eq!(
        row_value(&breakdown, "machine", "consumption"),
        "160%"
    );
    assert_eq!(
        row_value(&breakdown, "recipe", "craft time"),
        "0.5 s"
    );
    assert_eq!(
        row_value(&breakdown, "recipe", "productivity allowed"),
        "yes"
    );

    // 1.25 * 0.7 / 0.5 = 1.75 crafts per second, 20% productivity.
    let circuit = registry.entry("electronic-circuit").unwrap().glyph;
    let items = section(&breakdown, "items");
    let row = items
        .rows
        .iter()
        .find(|row| row.label.contains(circuit.to_string().as_str()))
        .unwrap();
    assert!(row.value.contains("produced 2.1"), "{}", row.value);

    // Power: 375 kW * 2.6 per machine; pollution: 2 * 2.6 * 1.2 per minute.
    assert_eq!(row_value(&breakdown, "power", "per machine"), "975 kW");
    assert_eq!(
        row_value(&breakdown, "pollution", "per machine"),
        "6.24/min"
    );
}

#[test]
fn machine_seconds_breakdown_has_machine_time_and_total_energy() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = run(&registry, "100 :iron-plate: in @:electric-furnace:");
    let breakdown = breakdown_of(&outputs[0], &settings, &registry);

    assert_eq!(
        row_value(&breakdown, "machine", "machine time"),
        "160 s"
    );
    assert_eq!(row_value(&breakdown, "power", "total energy"), "28.8 MJ");
    assert_eq!(
        row_value(&breakdown, "pollution", "total"),
        "5.333/min"
    );
}

#[test]
fn scalars_have_no_breakdown() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = run(&registry, "3 + 4");
    let breakdown = breakdown_of(&outputs[0], &settings, &registry);

    assert!(breakdown.sections.is_empty());
}
