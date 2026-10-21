use hactorio_core::{
    BaseUnit, EvalError, LineCache, LineOutput, Output, RecipeQuantity, Settings, StatementOutput,
    evaluate,
};
use hactorio_data::{Locale, Registry, postprocess};

const DUMP: &str = r#"{
  "item": {
    "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100, "icon": "a.png"},
    "iron-ore": {"type": "item", "name": "iron-ore", "stack_size": 50, "icon": "b.png"},
    "copper-plate": {"type": "item", "name": "copper-plate", "stack_size": 100, "icon": "b2.png"},
    "electronic-circuit": {"type": "item", "name": "electronic-circuit", "stack_size": 200, "icon": "c.png"},
    "catalyst": {"type": "item", "name": "catalyst", "stack_size": 1, "icon": "d.png"},
    "assembling-machine-3": {"type": "item", "name": "assembling-machine-3", "stack_size": 50, "icon": "e.png"},
    "electric-furnace": {"type": "item", "name": "electric-furnace", "stack_size": 50, "icon": "f.png"},
    "stone-furnace": {"type": "item", "name": "stone-furnace", "stack_size": 50, "icon": "g.png"}
  },
  "fluid": {
    "water": {"type": "fluid", "name": "water", "icon": "h.png"}
  },
  "module": {
    "speed-module-3": {
      "type": "module", "name": "speed-module-3", "category": "speed",
      "tier": 3, "stack_size": 50,
      "effect": {"speed": 0.5, "consumption": 0.7, "quality": -0.025}
    },
    "productivity-module-3": {
      "type": "module", "name": "productivity-module-3", "category": "productivity",
      "tier": 3, "stack_size": 50,
      "effect": {"speed": -0.15, "productivity": 0.1, "consumption": 0.8, "pollution": 0.1}
    }
  },
  "recipe": {
    "iron-plate": {
      "type": "recipe", "name": "iron-plate", "categories": ["smelting"],
      "energy_required": 3.2,
      "ingredients": [{"type": "item", "name": "iron-ore", "amount": 1}],
      "results": [{"type": "item", "name": "iron-plate", "amount": 1}],
      "allow_productivity": true
    },
    "copper-plate": {
      "type": "recipe", "name": "copper-plate", "categories": ["smelting"],
      "energy_required": 3.2,
      "ingredients": [{"type": "item", "name": "iron-ore", "amount": 1}],
      "results": [{"type": "item", "name": "copper-plate", "amount": 1}],
      "allow_productivity": true
    },
    "electronic-circuit": {
      "type": "recipe", "name": "electronic-circuit", "categories": ["crafting"],
      "energy_required": 0.5,
      "ingredients": [{"type": "item", "name": "iron-plate", "amount": 1}],
      "results": [{"type": "item", "name": "electronic-circuit", "amount": 1}],
      "allow_productivity": true
    },
    "catalyst-cycle": {
      "type": "recipe", "name": "catalyst-cycle", "categories": ["crafting"],
      "energy_required": 1,
      "ingredients": [{"type": "item", "name": "catalyst", "amount": 1}],
      "results": [{"type": "item", "name": "catalyst", "amount": 1}],
      "allow_productivity": false
    }
  },
  "assembling-machine": {
    "assembling-machine-3": {
      "type": "assembling-machine", "name": "assembling-machine-3",
      "crafting_categories": ["crafting"], "crafting_speed": 1.25, "module_slots": 4,
      "allowed_effects": ["speed", "productivity", "consumption"], "energy_usage": "375kW"
    }
  },
  "furnace": {
    "electric-furnace": {
      "type": "furnace", "name": "electric-furnace",
      "crafting_categories": ["smelting"], "crafting_speed": 2, "module_slots": 2,
      "allowed_effects": ["speed", "productivity", "consumption"], "energy_usage": "180kW"
    },
    "stone-furnace": {
      "type": "furnace", "name": "stone-furnace",
      "crafting_categories": ["smelting"], "crafting_speed": 1, "module_slots": 2,
      "allowed_effects": ["speed", "consumption"], "energy_usage": "90kW"
    }
  }
}"#;

fn registry() -> Registry {
    postprocess(DUMP, &Locale::empty()).unwrap()
}

fn run(registry: &Registry, tab: &str) -> Vec<StatementOutput> {
    evaluate(tab, &Settings::default(), registry, &mut LineCache::new())
}

fn run_with(registry: &Registry, settings: &Settings, tab: &str) -> Vec<StatementOutput> {
    evaluate(tab, settings, registry, &mut LineCache::new())
}

fn scalar_value(output: &StatementOutput) -> &hactorio_core::Value {
    match &output.output {
        LineOutput::Value(Output::Scalar(value)) => value,
        other => panic!("expected a scalar, got {other:?}"),
    }
}

fn scalar_coef(output: &StatementOutput) -> f64 {
    scalar_value(output).coef()
}

fn recipe_output(output: &StatementOutput) -> &hactorio_core::RecipeOutput {
    match &output.output {
        LineOutput::Value(Output::Recipe(recipe)) => recipe,
        other => panic!("expected a recipe, got {other:?}"),
    }
}

fn error(output: &StatementOutput) -> &EvalError {
    match &output.output {
        LineOutput::Error(error) => error,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn machine_count(output: &StatementOutput) -> f64 {
    match &output.output {
        LineOutput::Value(Output::Machine(machine)) => machine.count,
        other => panic!("expected a machine, got {other:?}"),
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{actual} is not close to {expected}"
    );
}

#[test]
fn evaluates_arithmetic() {
    let registry = registry();
    let outputs = run(&registry, "1 + 2 * 3\n-2^2\n2^-2\n4^0.5");

    assert_close(scalar_coef(&outputs[0]), 7.0);
    assert_close(scalar_coef(&outputs[1]), -4.0);
    assert_close(scalar_coef(&outputs[2]), 0.25);
    assert_close(scalar_coef(&outputs[3]), 2.0);
}

#[test]
fn variables_bind_and_resolve() {
    let registry = registry();
    let outputs = run(&registry, "a = 2\na * 3");

    assert_close(scalar_coef(&outputs[0]), 2.0);
    assert_close(scalar_coef(&outputs[1]), 6.0);
}

#[test]
fn unknown_variables_are_errors() {
    let registry = registry();
    let outputs = run(&registry, "nope + 1");
    assert!(error(&outputs[0]).message.contains("unknown variable"));
}

#[test]
fn redefinition_keeps_the_first_binding() {
    let registry = registry();
    let outputs = run(&registry, "a = 1\na = 2\na");

    assert!(error(&outputs[1]).message.contains("already defined"));
    assert_close(scalar_coef(&outputs[2]), 1.0);
}

#[test]
fn failed_definitions_poison_the_name() {
    let registry = registry();
    let outputs = run(
        &registry,
        "a = :iron-plate: + :iron-ore:\nb = a + 1\na",
    );

    let original = error(&outputs[0]).message.clone();
    assert!(original.contains("different units"), "{original}");
    assert_eq!(error(&outputs[1]).message, original);
    assert_eq!(error(&outputs[2]).message, original);
}

#[test]
fn belt_literals_use_the_stacking_setting() {
    let registry = registry();

    let stacked = run(&registry, "sgb :iron-plate:");
    assert_close(scalar_coef(&stacked[0]), 60.0 * 4.0);

    let unstacked = run_with(
        &registry,
        &Settings {
            belt_stacking: 1,
            ..Settings::default()
        },
        "sgb :iron-plate:",
    );
    assert_close(scalar_coef(&unstacked[0]), 60.0);
}

#[test]
fn machine_values_have_counts() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:assembling-machine-3:\n2 * @:assembling-machine-3:\n@:assembling-machine-3: + @:assembling-machine-3:\n@:assembling-machine-3: - @:assembling-machine-3:",
    );

    assert_close(machine_count(&outputs[0]), 1.0);
    assert_close(machine_count(&outputs[1]), 2.0);
    assert_close(machine_count(&outputs[2]), 2.0);
    assert_close(machine_count(&outputs[3]), 0.0);
}

#[test]
fn machine_configurations_must_match_to_add() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:assembling-machine-3: + @:assembling-machine-3: :speed-module-3:",
    );
    assert!(error(&outputs[0]).message.contains("different configurations"));
}

#[test]
fn machine_values_reject_unsupported_arithmetic() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:assembling-machine-3: * @:assembling-machine-3:\n@:assembling-machine-3: * 2 second",
    );

    assert!(error(&outputs[0]).message.contains("cannot multiply"));
    assert!(error(&outputs[1]).message.contains("dimensionless"));
}

#[test]
fn recipe_quantities_convert_between_counts_and_time() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:electric-furnace: making :iron-plate:\n\
         2 * @:electric-furnace: making :iron-plate:\n\
         3 * (@:electric-furnace: making :iron-plate:) * 5 second\n\
         (3 * (@:electric-furnace: making :iron-plate:) * 5 second) / 5 second",
    );

    assert_eq!(
        recipe_output(&outputs[0]).quantity,
        RecipeQuantity::Machines(1.0)
    );
    assert_eq!(
        recipe_output(&outputs[1]).quantity,
        RecipeQuantity::Machines(2.0)
    );
    assert_eq!(
        recipe_output(&outputs[2]).quantity,
        RecipeQuantity::MachineSeconds(15.0)
    );
    assert_eq!(
        recipe_output(&outputs[3]).quantity,
        RecipeQuantity::Machines(3.0)
    );
}

#[test]
fn recipe_time_arithmetic_is_checked() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:electric-furnace: making :iron-plate: / 5 second\n\
         (@:electric-furnace: making :iron-plate:) * 5 second * 5 second",
    );

    assert!(error(&outputs[0]).message.contains("machine count"));
    assert!(error(&outputs[1]).message.contains("machine time"));
}

#[test]
fn recipe_addition_checks_quantity_and_recipe() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:electric-furnace: making :iron-plate: + @:electric-furnace: making :iron-plate:\n\
         @:electric-furnace: making :iron-plate: + @:electric-furnace: making :copper-plate:\n\
         @:electric-furnace: making :iron-plate: + (@:electric-furnace: making :iron-plate:) * 2 second",
    );

    assert_eq!(
        recipe_output(&outputs[0]).quantity,
        RecipeQuantity::Machines(2.0)
    );
    assert!(error(&outputs[1]).message.contains("different outputs"));
    assert!(error(&outputs[2]).message.contains("machine time"));
}

#[test]
fn in_resolves_machine_counts_and_machine_seconds() {
    let registry = registry();
    let outputs = run(
        &registry,
        "100 :iron-plate: / second in @:electric-furnace:\n100 :iron-plate: in @:electric-furnace:",
    );

    assert_eq!(
        recipe_output(&outputs[0]).quantity,
        RecipeQuantity::Machines(160.0)
    );
    assert_eq!(
        recipe_output(&outputs[1]).quantity,
        RecipeQuantity::MachineSeconds(160.0)
    );

    let plate = BaseUnit::Item(registry.entry_id("iron-plate").unwrap());
    let ore = BaseUnit::Item(registry.entry_id("iron-ore").unwrap());
    let net = &recipe_output(&outputs[0]).net;
    assert_close(net[&plate], 100.0);
    assert_close(net[&ore], -100.0);
    assert_close(net[&BaseUnit::Craft], 100.0);
}

#[test]
fn in_with_craft_uses_the_craft_row() {
    let registry = registry();
    let outputs = run(
        &registry,
        "100 craft / second in @:electric-furnace: making :iron-plate:",
    );

    assert_eq!(
        recipe_output(&outputs[0]).quantity,
        RecipeQuantity::Machines(160.0)
    );
}

#[test]
fn craft_quantities_need_making() {
    let registry = registry();
    let outputs = run(&registry, "100 craft / second in @:electric-furnace:");
    assert!(error(&outputs[0]).message.contains("making"));
}

#[test]
fn in_requires_exactly_one_machine() {
    let registry = registry();
    let outputs = run(&registry, "100 :iron-plate: / second in 2 @:electric-furnace:");
    assert!(error(&outputs[0]).message.contains("exactly one machine"));
}

#[test]
fn in_rejects_other_dimensions() {
    let registry = registry();
    let outputs = run(
        &registry,
        "100 joule / second in @:electric-furnace:\n\
         3 in @:electric-furnace:\n\
         :iron-plate: * second in @:electric-furnace:",
    );

    for output in &outputs {
        assert!(matches!(output.output, LineOutput::Error(_)));
    }
}

#[test]
fn zero_coefficients_produce_infinity() {
    let registry = registry();
    let outputs = run(
        &registry,
        ":catalyst: / second in @:assembling-machine-3:\n\
         1 :catalyst: in @:assembling-machine-3:",
    );

    let machines = recipe_output(&outputs[0]).quantity;
    assert!(matches!(machines, RecipeQuantity::Machines(value) if value.is_infinite()));

    let seconds = recipe_output(&outputs[1]).quantity;
    assert!(matches!(seconds, RecipeQuantity::MachineSeconds(value) if value.is_infinite()));
}

#[test]
fn productivity_applies_to_products_only() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:electric-furnace: 2 :productivity-module-3: making :iron-plate:",
    );
    let output = recipe_output(&outputs[0]);

    // S = 2 * (1 + 2 * -0.15) = 1.4, T = 3.2, P = 0.2.
    let rate = 1.4 / 3.2;
    let plate = BaseUnit::Item(registry.entry_id("iron-plate").unwrap());
    let ore = BaseUnit::Item(registry.entry_id("iron-ore").unwrap());
    assert_close(output.net[&plate], rate * 1.2);
    assert_close(output.net[&ore], -rate);
    assert_close(output.net[&BaseUnit::Craft], rate);
    // Power = 180 kW * (1 + 2 * 0.8).
    assert_close(output.per_machine_power_w, 180_000.0 * 2.6);
}

#[test]
fn productivity_modules_need_a_recipe_that_allows_them() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:stone-furnace: :productivity-module-3: making :iron-plate:",
    );
    assert!(matches!(outputs[0].output, LineOutput::Error(_)));
}

#[test]
fn module_slots_are_enforced() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:electric-furnace: 3 :speed-module-3: making :iron-plate:",
    );
    assert!(error(&outputs[0]).message.contains("module slots"));
}

#[test]
fn machine_category_gates_modules_and_recipes() {
    let registry = registry();
    let outputs = run(
        &registry,
        "@:stone-furnace: :productivity-module-3:\n\
         @:assembling-machine-3: making :iron-plate:",
    );

    assert!(error(&outputs[0]).message.contains("does not accept"));
    assert!(error(&outputs[1]).message.contains("cannot craft"));
}

#[test]
fn comment_and_blank_lines_have_no_output() {
    let registry = registry();
    let outputs = run(&registry, "\n# note\n \n");

    for output in &outputs {
        assert_eq!(output.output, LineOutput::Nothing);
    }
}

#[test]
fn duplicate_lines_share_a_hash_but_not_a_line_index() {
    let registry = registry();
    let outputs = run(&registry, "1\n1\n# c");

    assert_eq!(outputs[0].source_hash, outputs[1].source_hash);
    assert_eq!(outputs[0].line, 0);
    assert_eq!(outputs[1].line, 1);
    assert_ne!(outputs[0].source_hash, outputs[2].source_hash);
}

#[test]
fn line_cache_prunes_unseen_lines() {
    let registry = registry();
    let mut cache = LineCache::new();

    evaluate("1", &Settings::default(), &registry, &mut cache);
    assert_eq!(cache.len(), 1);

    evaluate("2", &Settings::default(), &registry, &mut cache);
    assert_eq!(cache.len(), 1, "the first line is dropped");

    evaluate("2\n3", &Settings::default(), &registry, &mut cache);
    assert_eq!(cache.len(), 2);
}
