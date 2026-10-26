use hactorio_core::facet_eval::{LineOutput, evaluate};
use hactorio_core::{BaseUnit, Settings};
use hactorio_data::{Locale, Registry, postprocess};

const DUMP: &str = r#"{
  "item": {
    "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100},
    "copper-plate": {"type": "item", "name": "copper-plate", "stack_size": 100},
    "iron-gear-wheel": {"type": "item", "name": "iron-gear-wheel", "stack_size": 100},
    "transport-belt": {"type": "item", "name": "transport-belt", "stack_size": 100},
    "steel-chest": {"type": "item", "name": "steel-chest", "stack_size": 50},
    "assembling-machine-3": {"type": "item", "name": "assembling-machine-3", "stack_size": 50},
    "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}},
    "beacon": {"type": "item", "name": "beacon", "stack_size": 20}
  },
  "fluid": {"water": {"type": "fluid", "name": "water"}},
  "module": {
    "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}}
  },
  "recipe": {
    "iron-gear-wheel": {"type": "recipe", "name": "iron-gear-wheel", "categories": ["crafting"], "energy_required": 1.0, "ingredients": [{"type": "item", "name": "iron-plate", "amount": 2}], "results": [{"type": "item", "name": "iron-gear-wheel", "amount": 1}]}
  },
  "transport-belt": {
    "transport-belt": {"type": "transport-belt", "name": "transport-belt", "speed": 0.03125}
  },
  "container": {
    "steel-chest": {"type": "container", "name": "steel-chest", "inventory_size": 48}
  },
  "assembling-machine": {
    "assembling-machine-3": {"type": "assembling-machine", "name": "assembling-machine-3", "crafting_categories": ["crafting"], "crafting_speed": 1.0, "module_slots": 4, "allowed_effects": ["speed"], "energy_usage": "100kW"}
  },
  "beacon": {
    "beacon": {"type": "beacon", "name": "beacon", "module_slots": 2, "distribution_effectivity": 1.5, "profile": [1.0], "energy_usage": "480kW"}
  }
}"#;

fn registry() -> Registry {
    postprocess(DUMP, &Locale::empty()).unwrap()
}

#[test]
fn evaluates_new_syntax() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "3 :iron-plate:/s * (5 + 3) s\n:stack-size: :iron-plate:\n:stacked-belt: :transport-belt: :iron-plate:",
        &settings,
        &registry,
    );
    assert_eq!(outputs.len(), 3);
    let LineOutput::Value(rate) = &outputs[0].output else {
        panic!("expected rate");
    };
    assert_eq!(rate.item.as_ref().unwrap().coef(), 24.0);
    let LineOutput::Value(stack) = &outputs[1].output else {
        panic!("expected stack");
    };
    assert_eq!(stack.item.as_ref().unwrap().coef(), 100.0);
    let LineOutput::Value(belt) = &outputs[2].output else {
        panic!("expected stacked belt rate");
    };
    let item = belt.item.as_ref().unwrap();
    assert_eq!(item.coef(), 60.0);
    assert_eq!(item.exponent(BaseUnit::Second), -1);
}

#[test]
fn evaluates_machine_process_and_in() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "ma = :assembling-machine-3: with 2 :speed-module-3:\n\
         m2 = 2 :assembling-machine-3: with 2 :speed-module-3:\n\
         m2 making :iron-gear-wheel:\n\
         :iron-gear-wheel:/s in ma",
        &settings,
        &registry,
    );
    assert!(matches!(outputs[0].output, LineOutput::Value(_)));
    let LineOutput::Value(process) = &outputs[2].output else {
        panic!("expected process");
    };
    let process = process.process.as_ref().unwrap();
    assert_eq!(process.net[&BaseUnit::Craft], 4.0);
    let LineOutput::Value(invalue) = &outputs[3].output else {
        panic!("expected in");
    };
    let process = invalue.process.as_ref().unwrap();
    assert_eq!(process.kind, hactorio_core::facets::ExtentKind::Machines);
    assert_eq!(process.magnitude(), 0.5);
}

#[test]
fn variables_poison_after_evaluation_errors() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "x = :iron-plate: + :copper-plate:\ny = x + 2 :iron-plate:\nz = 3 :iron-plate:",
        &settings,
        &registry,
    );
    assert!(matches!(outputs[0].output, LineOutput::Error(_)));
    assert!(matches!(outputs[1].output, LineOutput::Error(_)));
    assert!(matches!(outputs[2].output, LineOutput::Value(_)));
}

#[test]
fn old_syntax_and_explicit_multiplication_rules() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "@:assembling-machine-3: 2:speed-module-3: making :iron-gear-wheel:\n\
         2 :iron-plate: 3 :copper-plate:\n\
         2 :iron-plate: 3 s\n\
         (:speed-module-3: + :beacon:) / 2",
        &settings,
        &registry,
    );
    assert!(matches!(outputs[0].output, LineOutput::Error(_)));
    assert!(matches!(outputs[1].output, LineOutput::Error(_)));
    assert!(matches!(outputs[2].output, LineOutput::Error(_)));
    assert!(matches!(outputs[3].output, LineOutput::Error(_)));
}

#[test]
fn definitions_shadow_earlier_bindings() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate("x = 3\nx = 5\nx", &settings, &registry);
    assert_eq!(outputs.len(), 3);
    for output in &outputs {
        assert!(
            matches!(output.output, LineOutput::Value(_)),
            "{:?}",
            output.output
        );
    }
    let value = outputs[2].value.as_ref().unwrap();
    assert_eq!(value.item.as_ref().unwrap().coef(), 5.0);
}

#[test]
fn compound_assignments_compose() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "x = 3\nx += 2\nx *= 4\nx -= 1\nx /= 5\nx = x + 1\nx",
        &settings,
        &registry,
    );
    assert_eq!(outputs.len(), 7);
    for output in &outputs {
        assert!(
            matches!(output.output, LineOutput::Value(_)),
            "{:?}",
            output.output
        );
    }
    let value = outputs[6].value.as_ref().unwrap();
    let coef = value.item.as_ref().unwrap().coef();
    assert!((coef - 4.8).abs() < 1e-9, "{coef}");
}

#[test]
fn compound_assignments_respect_facets_and_diagnostics() {
    let registry = registry();
    let settings = Settings::default();

    let outputs = evaluate(
        "rate = 3 :iron-plate: / second\nrate += 1 :iron-plate: / second\nrate",
        &settings,
        &registry,
    );
    assert_eq!(outputs.len(), 3);
    let value = outputs[2].value.as_ref().unwrap();
    let item = value.item.as_ref().unwrap();
    assert_eq!(item.coef(), 4.0);
    assert_eq!(item.exponent(BaseUnit::Second), -1);

    let outputs = evaluate("x += 1", &settings, &registry);
    let LineOutput::Error(error) = &outputs[0].output else {
        panic!("expected error, got {:?}", outputs[0].output);
    };
    assert_eq!(error.message, "unknown variable `x`");

    let outputs = evaluate(
        "x = 3 :iron-plate:\nx += 3 :copper-plate:",
        &settings,
        &registry,
    );
    let LineOutput::Error(error) = &outputs[1].output else {
        panic!("expected error, got {:?}", outputs[1].output);
    };
    assert!(
        error.message.contains("incompatible values"),
        "{}",
        error.message
    );
}

fn item_unit(registry: &Registry, name: &str) -> BaseUnit {
    BaseUnit::Item(registry.entry_id(name).unwrap())
}

fn process_of(
    output: &hactorio_core::facet_eval::StatementOutput,
) -> &hactorio_core::facets::ProcessFacet {
    output
        .value
        .as_ref()
        .and_then(|value| value.process.as_ref())
        .expect("expected a process")
}

fn error_message(output: &LineOutput) -> &str {
    match output {
        LineOutput::Error(error) => &error.message,
        other => panic!("expected error, got {other:?}"),
    }
}

#[test]
fn process_net_assignments_mutate_the_net() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "p = :iron-gear-wheel:/s in :assembling-machine-3:\n\
         p.:iron-plate: = 5 :iron-plate:/s\n\
         p.:iron-gear-wheel: += 2 :iron-gear-wheel:/s\n\
         p.:copper-plate: -= 3 :copper-plate:/s\n\
         p.:iron-plate: *= 4\n\
         p.:copper-plate: /= 3\n\
         p.crafts = 7/s\n\
         p\n\
         p.:iron-plate:",
        &settings,
        &registry,
    );
    assert_eq!(outputs.len(), 9);
    for output in &outputs {
        assert!(
            matches!(output.output, LineOutput::Value(_)),
            "{:?}",
            output.output
        );
    }
    let net = &process_of(&outputs[7]).net;
    assert_eq!(net[&item_unit(&registry, "iron-plate")], 20.0);
    assert_eq!(net[&item_unit(&registry, "iron-gear-wheel")], 3.0);
    assert_eq!(net[&item_unit(&registry, "copper-plate")], -1.0);
    assert_eq!(net[&BaseUnit::Craft], 7.0);

    let item = outputs[8].value.as_ref().unwrap().item.as_ref().unwrap();
    assert_eq!(item.coef(), 20.0);
    assert_eq!(item.exponent(BaseUnit::Second), -1);
}

#[test]
fn process_net_assignments_canonicalize_zero() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "p = :iron-gear-wheel:/s in :assembling-machine-3:\n\
         p.:iron-plate: = 0/s\n\
         p.crafts = 0/s",
        &settings,
        &registry,
    );
    for output in &outputs {
        assert!(
            matches!(output.output, LineOutput::Value(_)),
            "{:?}",
            output.output
        );
    }
    let net = &process_of(&outputs[1]).net;
    assert!(!net.contains_key(&item_unit(&registry, "iron-plate")));
    let net = &process_of(&outputs[2]).net;
    assert!(!net.contains_key(&BaseUnit::Craft));
}

#[test]
fn process_net_mul_and_div_skip_absent_units() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "p = :iron-gear-wheel:/s in :assembling-machine-3:\n\
         p.:copper-plate: *= 2\n\
         p.:copper-plate: /= 0\n\
         p",
        &settings,
        &registry,
    );
    for output in &outputs {
        assert!(
            matches!(output.output, LineOutput::Value(_)),
            "{:?}",
            output.output
        );
    }
    let net = &process_of(&outputs[3]).net;
    assert!(!net.contains_key(&item_unit(&registry, "copper-plate")));
    assert!(net.values().all(|amount| amount.is_finite()));
}

#[test]
fn process_net_assignments_reject_wrong_units() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "p = :iron-gear-wheel:/s in :assembling-machine-3:\n\
         q = p\n\
         r = p\n\
         v = p\n\
         t = :iron-gear-wheel: in :assembling-machine-3:\n\
         w = t\n\
         p.:iron-plate: = 5\n\
         q.:iron-plate: = 5 :copper-plate:/s\n\
         r.power = 5\n\
         v.:iron-plate: += 1 s\n\
         t.crafts = 5/s\n\
         w.:electric: = 5/s",
        &settings,
        &registry,
    );
    assert_eq!(
        error_message(&outputs[6].output),
        "expected `:iron-plate:/s` or `/s`"
    );
    assert_eq!(
        error_message(&outputs[7].output),
        "expected `:iron-plate:/s` or `/s`"
    );
    assert_eq!(
        error_message(&outputs[8].output),
        "expected a power value such as `100 kW`"
    );
    assert_eq!(
        error_message(&outputs[9].output),
        "expected `:iron-plate:/s` or `/s`"
    );
    assert_eq!(error_message(&outputs[10].output), "expected `craft`");
    assert_eq!(
        error_message(&outputs[11].output),
        "expected an energy value such as `100 kJ`"
    );
}

#[test]
fn process_power_and_energy_assignments() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "p = :assembling-machine-3: making :iron-gear-wheel:\n\
         p.power = 100 kW\n\
         p\n\
         p.power\n\
         p.:electric: += 1 kW\n\
         p.power *= 2\n\
         q = :iron-gear-wheel: in :assembling-machine-3:\n\
         q.:electric: = 100 kJ\n\
         q.:electric:\n\
         q\n\
         p.power = 0 W\n\
         p",
        &settings,
        &registry,
    );
    for output in &outputs {
        assert!(
            matches!(output.output, LineOutput::Value(_)),
            "{:?}",
            output.output
        );
    }
    assert_eq!(process_of(&outputs[2]).electric, 100_000.0);
    assert_eq!(
        outputs[3]
            .value
            .as_ref()
            .unwrap()
            .item
            .as_ref()
            .unwrap()
            .coef(),
        100_000.0
    );
    assert_eq!(process_of(&outputs[4]).electric, 101_000.0);
    assert_eq!(process_of(&outputs[5]).electric, 202_000.0);
    assert_eq!(process_of(&outputs[9]).electric, 100_000.0);
    assert_eq!(
        outputs[8]
            .value
            .as_ref()
            .unwrap()
            .item
            .as_ref()
            .unwrap()
            .coef(),
        100_000.0
    );
    assert_eq!(process_of(&outputs[11]).electric, 0.0);
}

#[test]
fn process_net_assigns_fluids() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "p = :iron-gear-wheel:/s in :assembling-machine-3:\n\
         p.:water: = 30 :water:/s\n\
         p.:water:",
        &settings,
        &registry,
    );
    for output in &outputs {
        assert!(
            matches!(output.output, LineOutput::Value(_)),
            "{:?}",
            output.output
        );
    }
    assert_eq!(
        process_of(&outputs[1]).net[&BaseUnit::Fluid(registry.entry_id("water").unwrap())],
        30.0
    );
    let item = outputs[2].value.as_ref().unwrap().item.as_ref().unwrap();
    assert_eq!(item.coef(), 30.0);
    assert_eq!(item.exponent(BaseUnit::Second), -1);
}

#[test]
fn process_crafts_assignment_rejects_multiple_machine_specs() {
    let registry = registry();
    let settings = Settings::default();
    let outputs = evaluate(
        "p = :iron-gear-wheel:/s in :assembling-machine-3:\n\
         q = :iron-gear-wheel:/s in (:assembling-machine-3: with :speed-module-3:)\n\
         p = p + q\n\
         p.crafts = 1/s",
        &settings,
        &registry,
    );
    for output in &outputs[..3] {
        assert!(
            matches!(output.output, LineOutput::Value(_)),
            "{:?}",
            output.output
        );
    }
    assert_eq!(
        error_message(&outputs[3].output),
        "`.crafts` cannot be assigned to a process with multiple machine configurations"
    );
}
