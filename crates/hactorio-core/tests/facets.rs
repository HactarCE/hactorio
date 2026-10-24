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
  "fluid": {},
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
    assert_eq!(
        invalue.process.as_ref().unwrap().extent,
        Some(hactorio_core::facets::Extent::Machines(0.5))
    );
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
