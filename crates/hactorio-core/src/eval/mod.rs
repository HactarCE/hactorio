//! The expression language specified by `SPEC.md`.
//!
//! Evaluation never blocks: a document is evaluated line by line, an error
//! attaches to the line that produced it, and a definition that fails poisons
//! its name (§2.5).

mod eval;
mod format;
mod lex;
mod parse;
mod process;
pub mod value;

pub use eval::{Evaluation, Output, evaluate, line_hash, process_resources, property_is_available};
pub use format::{format_facets, format_number, format_process_panel, format_value};
pub use parse::{ParseError, Span, definition_value_span, parse_line};
pub use value::{
    Amount, Comparison, Item, MultiValue, Quality, Quantity, RecipeRef, Resource, ResourceRef,
    Value, ValueError, is_zero,
};

#[cfg(test)]
pub(crate) fn test_registry() -> hactorio_data::Registry {
    use hactorio_data::{Locale, postprocess};

    const DUMP: &str = r#"{
      "item": {
        "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100},
        "carbonic-asteroid-crushing": {
          "type": "recipe",
          "name": "carbonic-asteroid-crushing",
          "categories": ["crushing"],
          "energy_required": 2.0,
          "ingredients": [{"type": "item", "name": "metallic-asteroid-chunk", "amount": 1}],
          "results": [{"type": "item", "name": "carbon", "amount": 1}]
        },
        "advanced-carbonic-asteroid-crushing": {
          "type": "recipe",
          "name": "advanced-carbonic-asteroid-crushing",
          "categories": ["crushing"],
          "energy_required": 5.0,
          "ingredients": [{"type": "item", "name": "metallic-asteroid-chunk", "amount": 1}],
          "results": [{"type": "item", "name": "carbon", "amount": 2}]
        },
        "casting-iron": {
          "type": "recipe",
          "name": "casting-iron",
          "categories": ["smelting"],
          "energy_required": 3.2,
          "ingredients": [{"type": "fluid", "name": "molten-iron", "amount": 20}],
          "results": [{"type": "item", "name": "iron-plate", "amount": 1}]
        },
        "copper-plate": {"type": "item", "name": "copper-plate", "stack_size": 100},
        "transport-belt": {"type": "item", "name": "transport-belt", "stack_size": 100},
        "foundry": {"type": "item", "name": "foundry", "stack_size": 20},
        "iron-ore": {"type": "item", "name": "iron-ore", "stack_size": 50},
        "copper-ore": {"type": "item", "name": "copper-ore", "stack_size": 50},
        "carbon": {"type": "item", "name": "carbon", "stack_size": 50},
        "crusher": {"type": "item", "name": "crusher", "stack_size": 10},
        "metallic-asteroid-chunk": {"type": "item", "name": "metallic-asteroid-chunk", "stack_size": 20},
        "beacon": {"type": "item", "name": "beacon", "stack_size": 20},
        "speed-module-2": {"type": "item", "name": "speed-module-2", "stack_size": 50},
        "productivity-module-1": {"type": "item", "name": "productivity-module-1", "stack_size": 50},
        "efficiency-module-1": {"type": "item", "name": "efficiency-module-1", "stack_size": 50},
        "quality-module-2": {"type": "item", "name": "quality-module-2", "stack_size": 50},
        "inserter": {"type": "item", "name": "inserter", "stack_size": 50},
        "bulk-inserter": {"type": "item", "name": "bulk-inserter", "stack_size": 50},
        "stack-inserter": {"type": "item", "name": "stack-inserter", "stack_size": 50},
        "steel-chest": {"type": "item", "name": "steel-chest", "stack_size": 50},
        "storage-tank": {"type": "item", "name": "storage-tank", "stack_size": 50},
        "pump": {"type": "item", "name": "pump", "stack_size": 50},
        "recycler": {"type": "item", "name": "recycler", "stack_size": 20},
        "assembling-machine": {
        "foundry": {"type": "assembling-machine", "name": "foundry", "crafting_categories": ["smelting"], "crafting_speed": 4.0, "module_slots": 4, "energy_usage": "2MW", "effect_receiver": {"base_effect": {"productivity": 0.5}}, "allowed_effects": ["speed", "productivity", "consumption", "pollution", "quality"]},
        "crusher": {"type": "assembling-machine", "name": "crusher", "crafting_categories": ["crushing"], "crafting_speed": 1.0, "module_slots": 2, "energy_usage": "540kW"},
        "recycler": {"type": "assembling-machine", "name": "recycler", "crafting_categories": ["recycling"], "crafting_speed": 1.0, "module_slots": 4, "energy_usage": "100kW", "allowed_effects": ["speed", "productivity", "consumption", "pollution", "quality"]}
      },
      "offshore-pump": {"type": "item", "name": "offshore-pump", "stack_size": 20},
        "barrel": {"type": "item", "name": "barrel", "stack_size": 10, "subgroup": "intermediate-product"},
        "crude-oil-barrel": {"type": "item", "name": "crude-oil-barrel", "stack_size": 10, "subgroup": "barrel"}
      },
      "fluid": {
        "molten-iron": {"type": "fluid", "name": "molten-iron"}
      },
      "recipe": {
        "iron-plate": {
          "type": "recipe",
          "name": "iron-plate",
          "categories": ["smelting"],
          "energy_required": 3.2,
          "allow_productivity": true,
          "ingredients": [{"type": "item", "name": "iron-ore", "amount": 1}],
          "results": [{"type": "item", "name": "iron-plate", "amount": 1}]
        },
        "carbonic-asteroid-crushing": {
          "type": "recipe",
          "name": "carbonic-asteroid-crushing",
          "categories": ["crushing"],
          "energy_required": 2.0,
          "ingredients": [{"type": "item", "name": "metallic-asteroid-chunk", "amount": 1}],
          "results": [{"type": "item", "name": "carbon", "amount": 1}]
        },
        "advanced-carbonic-asteroid-crushing": {
          "type": "recipe",
          "name": "advanced-carbonic-asteroid-crushing",
          "categories": ["crushing"],
          "energy_required": 5.0,
          "ingredients": [{"type": "item", "name": "metallic-asteroid-chunk", "amount": 1}],
          "results": [{"type": "item", "name": "carbon", "amount": 2}]
        },
        "casting-iron": {
          "type": "recipe",
          "name": "casting-iron",
          "categories": ["smelting"],
          "energy_required": 3.2,
          "ingredients": [{"type": "fluid", "name": "molten-iron", "amount": 20}],
          "results": [{"type": "item", "name": "iron-plate", "amount": 1}]
        },
        "copper-plate": {
          "type": "recipe",
          "name": "copper-plate",
          "categories": ["smelting"],
          "energy_required": 3.2,
          "allow_productivity": true,
          "ingredients": [{"type": "item", "name": "copper-ore", "amount": 1}],
          "results": [{"type": "item", "name": "copper-plate", "amount": 1}]
        },
        "iron-plate-recycling": {
          "type": "recipe",
          "name": "iron-plate-recycling",
          "categories": ["recycling"],
          "energy_required": 0.03125,
          "ingredients": [{"type": "item", "name": "iron-plate", "amount": 1}],
          "results": [{"type": "item", "name": "iron-plate", "amount": 1}],
          "hidden": true
        }
      },
      "transport-belt": {
        "transport-belt": {"type": "transport-belt", "name": "transport-belt", "speed": 0.03125}
      },
      "inserter": {
        "inserter": {"type": "inserter", "name": "inserter", "rotation_speed": 0.014, "bulk": false},
        "bulk-inserter": {"type": "inserter", "name": "bulk-inserter", "rotation_speed": 0.014, "bulk": true},
        "stack-inserter": {"type": "inserter", "name": "stack-inserter", "rotation_speed": 0.014, "bulk": true, "stack_size_bonus": 4}
      },
      "container": {
        "steel-chest": {"type": "container", "name": "steel-chest", "inventory_size": 48}
      },
      "storage-tank": {
        "storage-tank": {"type": "storage-tank", "name": "storage-tank", "fluid_box": {"volume": 25000}}
      },
      "pump": {
        "pump": {"type": "pump", "name": "pump", "pumping_speed": 0.5}
      },
      "module": {
        "speed-module-2": {"type": "module", "name": "speed-module-2", "category": "speed", "tier": 2, "stack_size": 50, "effect": {"speed": 0.3, "consumption": 0.7}},
        "productivity-module-1": {"type": "module", "name": "productivity-module-1", "category": "productivity", "tier": 1, "stack_size": 50, "effect": {"productivity": 0.04, "consumption": 0.4}},
        "efficiency-module-1": {"type": "module", "name": "efficiency-module-1", "category": "efficiency", "tier": 1, "stack_size": 50, "effect": {"consumption": -0.3}},
        "quality-module-2": {"type": "module", "name": "quality-module-2", "category": "quality", "tier": 2, "stack_size": 50, "effect": {"quality": 0.25}}
      },
      "beacon": {
        "beacon": {"type": "beacon", "name": "beacon", "module_slots": 2, "allowed_effects": ["speed", "consumption"], "distribution_effectivity": 1.5, "profile": [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0], "energy_usage": "480kW"}
      },
      "assembling-machine": {
        "foundry": {"type": "assembling-machine", "name": "foundry", "crafting_categories": ["smelting"], "crafting_speed": 4.0, "module_slots": 4, "energy_usage": "2MW", "effect_receiver": {"base_effect": {"productivity": 0.5}}, "allowed_effects": ["speed", "productivity", "consumption", "pollution", "quality"]}
      },
      "assembling-machine": {
        "foundry": {"type": "assembling-machine", "name": "foundry", "crafting_categories": ["smelting"], "crafting_speed": 4.0, "module_slots": 4, "energy_usage": "2MW", "effect_receiver": {"base_effect": {"productivity": 0.5}}, "allowed_effects": ["speed", "productivity", "consumption", "pollution", "quality"]},
        "crusher": {"type": "assembling-machine", "name": "crusher", "crafting_categories": ["crushing"], "crafting_speed": 1.0, "module_slots": 2, "energy_usage": "540kW"},
        "recycler": {"type": "assembling-machine", "name": "recycler", "crafting_categories": ["recycling"], "crafting_speed": 1.0, "module_slots": 4, "energy_usage": "100kW", "allowed_effects": ["speed", "productivity", "consumption", "pollution", "quality"]}
      },
      "offshore-pump": {
        "module": {
        "speed-module-2": {"type": "module", "name": "speed-module-2", "category": "speed", "tier": 2, "stack_size": 50, "effect": {"speed": 0.3, "consumption": 0.7}},
        "productivity-module-1": {"type": "module", "name": "productivity-module-1", "category": "productivity", "tier": 1, "stack_size": 50, "effect": {"productivity": 0.04, "consumption": 0.4}},
        "efficiency-module-1": {"type": "module", "name": "efficiency-module-1", "category": "efficiency", "tier": 1, "stack_size": 50, "effect": {"consumption": -0.3}},
        "quality-module-2": {"type": "module", "name": "quality-module-2", "category": "quality", "tier": 2, "stack_size": 50, "effect": {"quality": 0.25}}
      },
      "beacon": {
        "beacon": {"type": "beacon", "name": "beacon", "module_slots": 2, "allowed_effects": ["speed", "consumption"], "distribution_effectivity": 1.5, "profile": [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0], "energy_usage": "480kW"}
      },
      "assembling-machine": {
        "foundry": {"type": "assembling-machine", "name": "foundry", "crafting_categories": ["smelting"], "crafting_speed": 4.0, "module_slots": 4, "energy_usage": "2MW", "effect_receiver": {"base_effect": {"productivity": 0.5}}, "allowed_effects": ["speed", "productivity", "consumption", "pollution", "quality"]}
      },
      "assembling-machine": {
        "foundry": {"type": "assembling-machine", "name": "foundry", "crafting_categories": ["smelting"], "crafting_speed": 4.0, "module_slots": 4, "energy_usage": "2MW", "effect_receiver": {"base_effect": {"productivity": 0.5}}, "allowed_effects": ["speed", "productivity", "consumption", "pollution", "quality"]},
        "crusher": {"type": "assembling-machine", "name": "crusher", "crafting_categories": ["crushing"], "crafting_speed": 1.0, "module_slots": 2, "energy_usage": "540kW"},
        "recycler": {"type": "assembling-machine", "name": "recycler", "crafting_categories": ["recycling"], "crafting_speed": 1.0, "module_slots": 4, "energy_usage": "100kW", "allowed_effects": ["speed", "productivity", "consumption", "pollution", "quality"]}
      },
      "offshore-pump": {"type": "offshore-pump", "name": "offshore-pump", "pumping_speed": 20}
      },
      "technology": {
        "inserter-capacity-bonus-1": {"type": "technology", "effects": [{"type": "bulk-inserter-capacity-bonus", "modifier": 1}]},
        "inserter-capacity-bonus-2": {"type": "technology", "effects": [{"type": "inserter-stack-size-bonus", "modifier": 1}, {"type": "bulk-inserter-capacity-bonus", "modifier": 1}]},
        "transport-belt-capacity-1": {"type": "technology", "effects": [{"type": "belt-stack-size-bonus", "modifier": 1}]},
        "transport-belt-capacity-2": {"type": "technology", "effects": [{"type": "belt-stack-size-bonus", "modifier": 1}]},
        "stack-inserter": {"type": "technology", "effects": [{"type": "belt-stack-size-bonus", "modifier": 1}]}
      }
    }"#;

    postprocess(DUMP, &Locale::empty()).expect("test dump")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::format::format_process_panel;

    use crate::Settings;
    use crate::eval::value::{StackLimit, Stacks};
    use crate::settings::IdlePower;
    use hactorio_data::STACK_SIZE_GLYPH;

    fn number(text: &str) -> f64 {
        let registry = test_registry();
        let outputs = evaluate(text, &registry, &Settings::default(), &|| false).outputs;
        match &outputs[0] {
            Output::Value(value) => value
                .dimensionless()
                .unwrap_or_else(|| panic!("expected a dimensionless value, got {value:?}")),
            other => panic!("expected a value, got {other:?}"),
        }
    }

    fn coefficient(text: &str) -> f64 {
        let registry = test_registry();
        let outputs = evaluate(text, &registry, &Settings::default(), &|| false).outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected facets, got {:?}", outputs[0]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        let quantity = multi.quantity.clone().expect("a quantity");
        match quantity.amount {
            super::value::Amount::Number(value) => value,
            super::value::Amount::Resources(terms) => terms.values().sum(),
        }
    }

    fn fluid_glyph() -> char {
        hactorio_data::signal_by_id("fluid")
            .expect("a fluid signal")
            .glyph
    }

    fn full_belt_research() -> Settings {
        Settings {
            belt_stacking: 4,
            ..Settings::default()
        }
    }

    fn document(text: &str) -> Vec<Output> {
        evaluate(text, &test_registry(), &Settings::default(), &|| false).outputs
    }

    #[test]
    fn a_document_evaluates_line_by_line() {
        let outputs = document("x = 2\nx * 3");
        assert_eq!(outputs[0], Output::Value(Box::new(Value::number(2.0))));
        assert_eq!(outputs[1], Output::Value(Box::new(Value::number(6.0))));
    }

    #[test]
    fn blank_and_comment_lines_are_blank() {
        let outputs = document("\n# note\n1");
        assert_eq!(outputs[0], Output::Blank);
        assert_eq!(outputs[1], Output::Blank);
        assert_eq!(outputs[2], Output::Value(Box::new(Value::number(1.0))));
    }

    #[test]
    fn tokens_render_with_their_glyphs() {
        let registry = test_registry();
        let outputs = evaluate(":iron-plate:", &registry, &Settings::default(), &|| false).outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected a value, got {:?}", outputs[0]);
        };
        let glyph = registry
            .entry_by_index(registry.entry_id("iron-plate").unwrap())
            .glyph;
        assert_eq!(
            format_value(value, &Settings::default(), &registry),
            format!("1{glyph}")
        );
    }

    #[test]
    fn a_document_renders_its_results() {
        let registry = test_registry();
        let glyph = |id: &str| {
            registry
                .entry_by_index(registry.entry_id(id).unwrap())
                .glyph
        };
        let (iron, copper) = (glyph("iron-plate"), glyph("copper-plate"));

        let text = "x = 2:iron-plate:\nx + 1:copper-plate:\n:iron-plate: .:stack-size:\n(2:iron-plate:) .:stack-size:\n:stacked-belt: :transport-belt:";
        let outputs = evaluate(text, &registry, &Settings::default(), &|| false).outputs;
        let rendered: Vec<String> = outputs
            .iter()
            .map(|output| match output {
                Output::Value(value) => format_value(value, &Settings::default(), &registry),
                Output::Error { message, .. } => format!("error: {message}"),
                Output::Blank => String::new(),
                Output::None => "\u{2014}".to_string(),
            })
            .collect();
        println!("{rendered:#?}");

        assert_eq!(rendered[0], format!("2{iron}"));
        assert_eq!(rendered[1], format!("1{copper} + 2{iron}"));
        assert_eq!(rendered[2], "100");
        // Scaling drops the reference facets (D28), so the stack size is gone.
        assert_eq!(
            rendered[3],
            "error: `.:stack-size:` needs a resource facet (quantity)"
        );
        assert_eq!(
            rendered[4],
            format!("15{}{}", hactorio_data::STACK_SIZE_GLYPH, "₄/s")
        );
    }

    #[test]
    fn belts_apply_and_stack() {
        let registry = test_registry();
        let iron = registry
            .entry_by_index(registry.entry_id("iron-plate").unwrap())
            .glyph;

        // A yellow belt moves 15 items/s at a stack cap of one.
        let outputs = evaluate(
            ":transport-belt: :iron-plate:",
            &registry,
            &Settings::default(),
            &|| false,
        )
        .outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected a value, got {:?}", outputs[0]);
        };
        assert_eq!(
            format_value(value, &Settings::default(), &registry),
            format!("15{iron}/s")
        );

        // With belt-stacking research complete, stacked belts carry four per
        // slot, so 60 items/s.
        let researched = full_belt_research();
        let outputs = evaluate(
            ":stacked-belt: :transport-belt: :iron-plate:",
            &registry,
            &researched,
            &|| false,
        )
        .outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected a value, got {:?}", outputs[0]);
        };
        assert_eq!(
            format_value(value, &Settings::default(), &registry),
            format!("60{iron}/s")
        );

        // Stacking an already stacked rate is an error: its limit is not one.
        // At level one stacking is a no-op, so the error needs research.
        let outputs = evaluate(
            ":stacked-belt: (:stacked-belt: :transport-belt:)",
            &registry,
            &researched,
            &|| false,
        )
        .outputs;
        assert!(matches!(outputs[0], Output::Error { .. }));

        // A rate times a time is a capacity: 15 stacks/s at cap one, for 8 s.
        let outputs = evaluate(
            "(:transport-belt: * 8s).capacity",
            &registry,
            &Settings::default(),
            &|| false,
        )
        .outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected a value, got {:?}", outputs[0]);
        };
        assert_eq!(
            format_value(value, &Settings::default(), &registry),
            format!("120{}₁", hactorio_data::STACK_SIZE_GLYPH)
        );
    }

    #[test]
    fn a_rate_applies_to_a_prototype_not_to_a_counted_amount() {
        let registry = test_registry();
        let settings = Settings::default();
        let glyph = |id: &str| {
            registry
                .entry_by_index(registry.entry_id(id).unwrap())
                .glyph
        };
        let (iron, fluid) = (glyph("iron-plate"), glyph("molten-iron"));
        let run = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };

        // The right operand is an item or fluid prototype, so a rate and a
        // capacity both apply.
        assert_eq!(run(":transport-belt: :iron-plate:"), format!("15{iron}/s"));
        assert_eq!(run(":steel-chest: :iron-plate:"), format!("4.8k{iron}"));
        assert_eq!(run(":pump: :molten-iron:"), format!("30{fluid}/s"));

        // A qualified prototype keeps its quality.
        let rare = Quality::Rare.glyph();
        assert_eq!(
            run(":transport-belt: :rare::iron-plate:"),
            format!("15{rare}{iron}/s")
        );

        // A counted amount has lost its prototype (§5.1), so the pair matches
        // no rule and the statement errors.
        let outputs = evaluate(
            ":transport-belt: 2 :iron-plate:",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        assert!(
            matches!(outputs[0], Output::Error { .. }),
            "{:?}",
            outputs[0]
        );
    }

    #[test]
    fn entity_tokens_carry_rates_and_capacities() {
        let registry = test_registry();
        let settings = Settings::default();
        let fluid = fluid_glyph();
        let run = |text: &str, settings: &Settings| {
            let outputs = evaluate(text, &registry, settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => format_value(value, settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };

        // A basic inserter moves one item per cycle, at cap one.
        assert_eq!(
            run(":inserter: .rate", &settings),
            format!("0.857{}₁/s", STACK_SIZE_GLYPH)
        );
        assert_eq!(
            run(":bulk-inserter: .rate", &settings),
            format!("0.857{}₂/s", STACK_SIZE_GLYPH)
        );

        // Capacity is the number of stacks a container holds.
        assert_eq!(
            run(":steel-chest: .capacity", &settings),
            format!("48{STACK_SIZE_GLYPH}")
        );
        assert_eq!(
            run(":storage-tank: .capacity", &settings),
            format!("25k{fluid}")
        );
        assert_eq!(run(":pump: .rate", &settings), format!("30{fluid}/s"));
        // The offshore pump is its own prototype: 20 fluid per tick.
        assert_eq!(
            run(":offshore-pump: .rate", &settings),
            format!("1.2k{fluid}/s")
        );
        // Barrels hold one fluid slot of 50, empty or filled.
        assert_eq!(
            run(":crude-oil-barrel: .capacity", &settings),
            format!("50{fluid}")
        );
        assert_eq!(run(":barrel: .capacity", &settings), format!("50{fluid}"));

        // Research reaches the capacity, and an over-large level is clamped to
        // the levels the registry has.
        let researched = Settings {
            inserter_capacity: 2,
            ..Settings::default()
        };
        let level_two = format!("0.857{}₂/s", STACK_SIZE_GLYPH);
        assert_eq!(run(":inserter: .rate", &researched), level_two);
        let over = Settings {
            inserter_capacity: 99,
            ..Settings::default()
        };
        assert_eq!(run(":inserter: .rate", &over), level_two);
    }

    #[test]
    fn multiplication_drops_a_rate_it_cannot_apply() {
        let registry = test_registry();
        let settings = Settings::default();
        let rendered = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => Some(format_value(value, &settings, &registry)),
                Output::Error { .. } => None,
                other => panic!("expected a value or an error, got {other:?}"),
            }
        };

        // A dimensionless factor scales the rate.
        assert_eq!(
            rendered("(:transport-belt: * 2).rate"),
            Some(format!("30{STACK_SIZE_GLYPH}₁/s"))
        );
        // A time turns the rate into a capacity and leaves no rate behind.
        assert_eq!(rendered("(:transport-belt: * 2s).rate"), None);
        assert_eq!(
            rendered("(:transport-belt: * 2s).capacity"),
            Some(format!("30{STACK_SIZE_GLYPH}₁"))
        );
        // A non-time quantity drops the rate and cannot multiply the bundles.
        assert_eq!(rendered("(:transport-belt: * 2:iron-plate:).rate"), None);
        assert_eq!(rendered(":transport-belt: * 2:iron-plate:"), None);
        // A capacity operand has no rule with a rate either.
        assert_eq!(rendered("(:transport-belt: * :steel-chest:).rate"), None);
    }

    #[test]
    fn qualities_flow_through_quantities() {
        let registry = test_registry();
        let iron = registry.entry_id("iron-plate").unwrap();
        let _ = &registry;
        let bundle = |text: &str| {
            let outputs = evaluate(text, &registry, &Settings::default(), &|| false).outputs;
            let Output::Value(value) = &outputs[0] else {
                panic!("expected facets, got {:?}", outputs[0]);
            };
            let Value::Multi(multi) = &**value else {
                unreachable!()
            };
            multi.quantity.clone().expect("a quantity")
        };

        // Two qualities of one item live in one bundle as separate terms.
        let mixed = bundle("2:iron-plate: + 3:rare::iron-plate:");
        let super::value::Amount::Resources(terms) = &mixed.amount else {
            panic!("expected a bundle");
        };
        assert_eq!(terms.len(), 2);
        assert_eq!(
            terms[&Resource::Item(Item {
                id: iron,
                quality: Quality::Normal
            })],
            2.0
        );
        assert_eq!(
            terms[&Resource::Item(Item {
                id: iron,
                quality: Quality::Rare
            })],
            3.0
        );

        // Quality is a first-class value with its level and multiplier.
        assert_eq!(number(":rare: .level"), 2.0);
        assert_eq!(number(":legendary: .multiplier"), 2.5);

        // The operand promotes a token, and selection picks one quality out.
        assert_eq!(coefficient("(:epic: :iron-plate:).:epic::iron-plate:"), 1.0);
        assert_eq!(
            coefficient("(:epic: :iron-plate:).:normal::iron-plate:"),
            0.0
        );
    }

    #[test]
    fn division_scales_by_a_scalar_but_not_the_other_way_round() {
        let registry = test_registry();
        let settings = Settings::default();
        let rendered = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => Some(format_value(value, &settings, &registry)),
                Output::Error { .. } => None,
                other => panic!("expected a value or an error, got {other:?}"),
            }
        };

        assert_eq!(
            rendered(":transport-belt: .rate / 2"),
            Some(format!("7.5{STACK_SIZE_GLYPH}₁/s"))
        );
        assert_eq!(
            rendered(":transport-belt: .capacity / 2"),
            Some(format!("4{STACK_SIZE_GLYPH}₁"))
        );
        // A scalar cannot be divided by a rate or a capacity.
        assert_eq!(rendered("2 / :transport-belt: .rate"), None);
        assert_eq!(rendered("2 / :transport-belt: .capacity"), None);
    }

    #[test]
    fn a_scalar_scales_every_scalable_facet() {
        let registry = test_registry();
        let settings = Settings::default();
        let rendered = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        assert_eq!(
            rendered("2 * :transport-belt: .rate"),
            format!("30{STACK_SIZE_GLYPH}₁/s")
        );
        assert_eq!(
            rendered(":transport-belt: .rate * 2"),
            format!("30{STACK_SIZE_GLYPH}₁/s")
        );
        assert_eq!(
            rendered(":transport-belt: .rate * 2"),
            rendered("2 :transport-belt: .rate")
        );
    }

    #[test]
    fn stacking_collapses_rate_and_capacity_together() {
        let registry = test_registry();
        let outputs = evaluate(
            ":stacked-belt: :transport-belt:",
            &registry,
            &full_belt_research(),
            &|| false,
        )
        .outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected facets, got {:?}", outputs[0]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        assert_eq!(
            multi.stack_rate,
            Some(Stacks::items(StackLimit::Limit(4), 15.0))
        );
        assert_eq!(
            multi.stack_capacity,
            Some(Stacks::items(StackLimit::Limit(4), 8.0))
        );
    }

    #[test]
    fn machines_and_configs_render_round_trippably() {
        let registry = test_registry();
        let settings = Settings::default();
        let glyph = |id: &str| {
            registry
                .entry_by_index(registry.entry_id(id).unwrap())
                .glyph
        };
        let (foundry, module, beacon) =
            (glyph("foundry"), glyph("speed-module-2"), glyph("beacon"));
        let rendered = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };

        assert_eq!(rendered(":foundry:"), foundry.to_string());
        assert_eq!(
            rendered(":foundry: 2 :speed-module-2:"),
            format!("{foundry}({module}{module})")
        );
        assert_eq!(
            rendered(":beacon: 2 :speed-module-2:"),
            format!("{beacon}{module}{module}")
        );
        // Filling a beacon keys on the prototype, not on a one-beacon config,
        // so a counted beacon still takes its module list (§5.2).
        assert_eq!(
            rendered("2 :beacon: 2 :speed-module-2:"),
            format!("2{beacon}{module}{module}")
        );
        // A config with both modules and a beacon group needs `+`: a chain
        // would bind the count rightward (D21).
        assert_eq!(
            rendered(":foundry: (2 :speed-module-2: + 2 :beacon:)"),
            format!("{foundry}({module}{module}+2{beacon})")
        );

        // A rendered machine parses back to an equal value.
        let original = rendered(":foundry: (2 :speed-module-2: + 2 :beacon:)");
        assert_eq!(rendered(&original), original);

        // Config juxtaposition may only build a module list or fill an empty
        // beacon; anything else needs `+`.
        assert_eq!(
            rendered(":speed-module-2: :speed-module-2:"),
            format!("{module}{module}")
        );
        // `+` is the only way to combine a module list with a beacon group;
        // the item quantities add alongside, so check the config itself.
        let outputs = evaluate(
            ":speed-module-2: + 2 :beacon:",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected facets, got {:?}", outputs[0]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        let config = multi.config.clone().expect("a config");
        assert_eq!(config.modules.len(), 1);
        assert_eq!(config.total_beacons(), 2);
        assert!(config.beacons.keys().all(|group| group.modules.is_empty()));
        for refused in [
            ":speed-module-2: 2 :beacon:",
            ":speed-module-2: :beacon:",
            "(:beacon: :speed-module-2:) :speed-module-2:",
            ":beacon: :beacon:",
        ] {
            let outputs = evaluate(refused, &registry, &settings, &|| false).outputs;
            assert!(
                matches!(outputs[0], Output::Error { .. }),
                "`{refused}` should be refused, got {:?}",
                outputs[0]
            );
        }

        // Machine signal selectors.
        assert_eq!(number(":foundry: .:module-slots:"), 4.0);
        assert_eq!(number(":foundry: .:speed:"), 4.0);
        assert_eq!(number("(:foundry: 2 :speed-module-2:).:speed:"), 6.4);

        // Machines add when their prototype and config match.
        assert_eq!(rendered("2:foundry: + 3:foundry:"), format!("5{foundry}"));
        assert_eq!(rendered("(2:foundry: + 3:foundry:).machines"), "5");
        let _ = module;
    }

    /// Every example presented as fact, pinned to what the code does. The
    /// renderings are normalised to plaintext so the expectations are
    /// readable and independent of glyph assignment.
    #[test]
    fn documented_examples() {
        let registry = test_registry();
        let default = Settings::default();
        let researched = full_belt_research();
        let unstacked = Settings {
            belt_stacking: 1,
            ..Settings::default()
        };

        let render = |text: &str, settings: &Settings| -> String {
            let outputs = evaluate(text, &registry, settings, &|| false).outputs;
            let rendered = match &outputs[0] {
                Output::Value(value) => format_value(value, settings, &registry),
                Output::Error { message, .. } => format!("ERROR: {message}"),
                other => format!("{other:?}"),
            };
            let mut readable = rendered;
            for (glyph, name) in [
                ('\u{f8f0}', "item"),
                ('\u{f8f1}', "fluid"),
                ('\u{f8f2}', "recycling"),
                ('\u{f8ff}', "stack-size"),
            ] {
                readable = readable.replace(glyph, &format!(":{name}:"));
            }
            crate::to_plaintext(&readable, &registry)
        };

        for (text, settings, expected) in [
            // Belts, alone and stacked. Stacking is researched, so the default
            // document stacks at one.
            (
                ":transport-belt: :iron-plate:",
                &default,
                "15:iron-plate:/s",
            ),
            (
                ":stacked-belt: :transport-belt: :iron-plate:",
                &unstacked,
                "15:iron-plate:/s",
            ),
            (
                ":stacked-belt: :transport-belt: :iron-plate:",
                &researched,
                "60:iron-plate:/s",
            ),
            (
                ":stacked-belt: :transport-belt:",
                &unstacked,
                "15:stack-size:₁/s",
            ),
            (
                ":stacked-belt: :transport-belt:",
                &researched,
                "15:stack-size:₄/s",
            ),
            (
                "(:transport-belt: * 8s).capacity",
                &default,
                "120:stack-size:₁",
            ),
            (":transport-belt: * 8s", &default, "8:transport-belt:s"),
            ("2 * :transport-belt: .rate", &default, "30:stack-size:₁/s"),
            (":transport-belt: .rate * 2", &default, "30:stack-size:₁/s"),
            (":transport-belt: .rate / 2", &default, "7.5:stack-size:₁/s"),
            (":transport-belt: .capacity / 2", &default, "4:stack-size:₁"),
            // Entities.
            (":inserter: .rate", &default, "0.857:stack-size:₁/s"),
            (":bulk-inserter: .rate", &default, "0.857:stack-size:₂/s"),
            (":stack-inserter: .rate", &default, "0.857:stack-size:₆/s"),
            (":steel-chest: .capacity", &default, "48:stack-size:"),
            (":storage-tank: .capacity", &default, "25k:fluid:"),
            (":pump: .rate", &default, "30:fluid:/s"),
            (":offshore-pump: .rate", &default, "1.2k:fluid:/s"),
            (":barrel: .capacity", &default, "50:fluid:"),
            (":crude-oil-barrel: .capacity", &default, "50:fluid:"),
            // Machines and configs.
            (":foundry:", &default, ":foundry:"),
            (
                ":foundry: 2 :speed-module-2:",
                &default,
                ":foundry:(:speed-module-2::speed-module-2:)",
            ),
            (
                ":beacon: 2 :speed-module-2:",
                &default,
                ":beacon::speed-module-2::speed-module-2:",
            ),
            (
                ":foundry: (2 :speed-module-2: + 2 :beacon:)",
                &default,
                ":foundry:(:speed-module-2::speed-module-2:+2:beacon:)",
            ),
            ("2:foundry: + 3:foundry:", &default, "5:foundry:"),
            ("(2:foundry: + 3:foundry:).machines", &default, "5"),
            (
                ":speed-module-2: :speed-module-2:",
                &default,
                ":speed-module-2::speed-module-2:",
            ),
            (
                ":beacon: :speed-module-2:",
                &default,
                ":beacon::speed-module-2:",
            ),
            // A general config addition keeps the item quantities, so the
            // quantity facet is what renders.
            (
                ":speed-module-2: + 2 :beacon:",
                &default,
                "2:beacon: + 1:speed-module-2:",
            ),
            // Quality.
            (":rare: .level", &default, "2"),
            (":legendary: .multiplier", &default, "2.5"),
            (
                "(1:iron-plate: + 2:rare::iron-plate:).:normal::iron-plate:",
                &default,
                "1:iron-plate:",
            ),
            (
                "(:epic: :iron-plate:).:epic::iron-plate:",
                &default,
                "1:epic::iron-plate:",
            ),
        ] {
            assert_eq!(render(text, settings), expected, "for `{text}`");
        }

        // The shapes juxtaposition cannot build.
        for refused in [
            ":speed-module-2: 2 :beacon:",
            ":speed-module-2: :beacon:",
            "(:beacon: :speed-module-2:) :speed-module-2:",
            ":beacon: :beacon:",
            "2 / :transport-belt: .rate",
        ] {
            assert!(
                render(refused, &default).starts_with("ERROR: "),
                "`{refused}` should be refused"
            );
        }
    }

    #[test]
    fn making_produces_a_process_with_rates() {
        let registry = test_registry();
        let settings = Settings::default();
        let render = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        let iron = registry
            .entry_by_index(registry.entry_id("iron-plate").unwrap())
            .glyph;
        let ore = registry
            .entry_by_index(registry.entry_id("iron-ore").unwrap())
            .glyph;

        // The foundry runs at speed 4, so 3.2 s per craft is 1.25 crafts/s.
        assert_eq!(
            render("(:foundry: making (:normal: :iron-plate:)).out.:iron-plate:"),
            format!("1.875{iron}/s")
        );
        assert_eq!(
            render("(:foundry: making (:normal: :iron-plate:)).net.:iron-ore:"),
            format!("-1.25{ore}/s")
        );
        assert_eq!(
            render("(:foundry: making (:normal: :iron-plate:)).machines"),
            "1"
        );
        assert_eq!(
            render("(2 :foundry: making (:normal: :iron-plate:)).out.:iron-plate:"),
            format!("3.75{iron}/s")
        );

        // Sizing: 540 iron plates per second needs 540 / 1.25 machines.
        assert_eq!(
            render("540 :iron-plate:/s from (:foundry: making (:normal: :iron-plate:))"),
            format!("288 · -360{ore}/s · 540{iron}/s")
        );
        assert_eq!(
            render("(540 :iron-plate:/s from (:foundry: making (:normal: :iron-plate:))).machines"),
            "288"
        );

        // An amount rather than a rate gives machine-seconds, which has no
        // machine count.
        let outputs = evaluate(
            "(100 :iron-plate: from (:foundry: making (:normal: :iron-plate:))).machines",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        assert!(matches!(outputs[0], Output::Error { .. }));

        // An unspecified recipe quality expands to five rows, so the totals
        // are one per quality.
        let outputs = evaluate(
            "(:foundry: making :iron-plate:).out.:iron-plate:",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected facets, got {:?}", outputs[0]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        let quantity = multi.quantity.clone().expect("a quantity");
        let super::value::Amount::Resources(terms) = &quantity.amount else {
            panic!("expected a bundle");
        };
        assert_eq!(terms.len(), 5);

        // Module and beacon effects reach the rates: two speed modules give
        // +0.6, and one beacon holding one module gives 0.3 * 1.5 = +0.45, so
        // the foundry runs at 4 * 2.05 = 8.2 and 8.2 / 3.2 crafts per second.
        assert_eq!(
            render(
                "(:foundry: (2 :speed-module-2: + :beacon: :speed-module-2:) \
                    making (:normal: :iron-plate:)).out.:iron-plate:"
            ),
            format!("3.844{iron}/s")
        );
        // The foundry draws 2 MW.
        assert_eq!(render("(2 :foundry:).power"), "4MW");
        assert_eq!(
            render("(100 :iron-plate: from (:foundry: making (:normal: :iron-plate:))).energy"),
            "106.667MJ"
        );
        assert_eq!(
            render("(2 :foundry: making (:normal: :iron-plate:)).:electric:"),
            "4MW"
        );
        // Productivity is shared across rows and includes the foundry's own
        // base +50%, plus whatever its modules give.
        assert_eq!(
            render("(:foundry: making (:normal: :iron-plate:)).productivity"),
            "1.5"
        );
        assert_eq!(render(":foundry: .productivity"), "1.5");
        assert_eq!(
            render("(:foundry: :productivity-module-1:).productivity"),
            "1.54"
        );

        // A beacon holds only modules it allows: speed and efficiency, never
        // productivity or quality, and never more than its slots.
        let efficiency = registry
            .entry_by_index(registry.entry_id("efficiency-module-1").unwrap())
            .glyph;
        let beacon = registry
            .entry_by_index(registry.entry_id("beacon").unwrap())
            .glyph;
        assert_eq!(
            render(":beacon: :efficiency-module-1:"),
            format!("{beacon}{efficiency}")
        );
        for refused in [
            ":beacon: :productivity-module-1:",
            ":beacon: 3 :speed-module-2:",
        ] {
            let outputs = evaluate(refused, &registry, &settings, &|| false).outputs;
            assert!(
                matches!(outputs[0], Output::Error { .. }),
                "`{refused}` should be refused, got {:?}",
                outputs[0]
            );
        }

        // A machine cannot make a recipe outside its categories.
        let outputs = evaluate(
            ":foundry: making (:recycling: :iron-plate:)",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        assert!(matches!(outputs[0], Output::Error { .. }));
    }

    #[test]
    fn sizing_by_several_resources_uses_several_recipes() {
        let registry = test_registry();
        let settings = Settings::default();
        let document = "r = (:foundry: making (:normal: :iron-plate:)) + (:foundry: making (:normal: :copper-plate:))\n100 :iron-plate:/s + 50 :copper-plate:/s from r";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[1] else {
            panic!("expected a process, got {:?}", outputs[1]);
        };
        let Value::Process(process) = &**value else {
            unreachable!()
        };
        assert_eq!(process.rows.len(), 2, "one row per resource");
        // Each recipe is 3.2 s at speed 4 with the foundry's +50%, so 1.875/s
        // of each product: 100/1.875 and 50/1.875 machines.
        let total: f64 = process.rows.values().sum();
        assert!((total - (100.0 + 50.0) / 1.875).abs() < 1e-9, "{total}");
    }

    #[test]
    fn sizing_by_nothing_is_an_empty_process() {
        let registry = test_registry();
        let settings = Settings::default();
        let document = "0 :iron-plate:/s from :foundry:";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected a process, got {:?}", outputs[0]);
        };
        let Value::Process(process) = &**value else {
            unreachable!()
        };
        assert!(process.rows.is_empty());
        assert_eq!(process.extent, crate::eval::value::Extent::Unspecified);
        assert_eq!(process.machines(), 0.0);

        // An empty process is the identity, so a loop accumulator survives it.
        let document = "p = 1 :foundry: making (:normal: :iron-plate:)\np += 0 :iron-plate:/s from :foundry:\np.machines";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        match &outputs[2] {
            Output::Value(value) => assert_eq!(value.dimensionless(), Some(1.0)),
            other => panic!("expected a value, got {other:?}"),
        }
    }

    #[test]
    fn a_fluid_fed_recipe_has_only_normal_quality() {
        let registry = test_registry();
        let settings = Settings::default();

        let making = evaluate(
            ":foundry: making :casting-iron:",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        let Output::Value(value) = &making[0] else {
            panic!("expected a process, got {:?}", making[0]);
        };
        let Value::Process(process) = &**value else {
            unreachable!()
        };
        assert_eq!(process.rows.len(), 1);

        // There is no rare iron plate to be had from it.
        let outputs = evaluate(
            ":rare::iron-plate: from (:foundry: making :casting-iron:)",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        assert!(
            matches!(outputs[0], Output::Error { .. }),
            "{:?}",
            outputs[0]
        );

        // Asking for the rare recipe outright is an error too.
        let outputs = evaluate(
            ":foundry: making (:rare: :casting-iron:)",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        assert!(
            matches!(outputs[0], Output::Error { .. }),
            "{:?}",
            outputs[0]
        );
    }

    #[test]
    fn scaling_a_process_picks_one_recipe() {
        let registry = test_registry();
        let settings = Settings::default();
        let machines = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            let Output::Value(value) = &outputs[0] else {
                panic!("expected a process, got {:?}", outputs[0]);
            };
            let Value::Process(process) = &**value else {
                unreachable!()
            };
            (process.rows.len(), process.machines())
        };

        // `making :iron-plate:` has five quality rows; a counted machine picks
        // the common one, and so does scaling a number into it.
        assert_eq!(machines("2 :foundry: making :iron-plate:"), (1, 2.0));
        assert_eq!(machines("2 * (:foundry: making :iron-plate:)"), (1, 2.0));
        // One row already, so nothing has to be chosen.
        assert_eq!(
            machines("2 * (:foundry: making (:normal: :iron-plate:))"),
            (1, 2.0)
        );
    }

    #[test]
    fn several_recipes_for_one_item_ask_for_making() {
        let registry = test_registry();
        let settings = Settings::default();
        // Two crushing recipes make carbon, as in the game, so inference has
        // no single answer.
        let outputs = evaluate(
            "100 :carbon:/s from :crusher:",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        match &outputs[0] {
            Output::Error { message, .. } => {
                assert!(message.contains("several recipes"), "{message}")
            }
            other => panic!("expected an error, got {other:?}"),
        }

        // Naming one works, and the two recipes size differently.
        let one = evaluate(
            "100 :carbon:/s from (:crusher: making :carbonic-asteroid-crushing:)",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        let Output::Value(value) = &one[0] else {
            panic!("expected a process, got {:?}", one[0]);
        };
        let Value::Process(process) = &**value else {
            unreachable!()
        };
        // 1 craft per 2 s at speed 1 is 0.5 carbon/s, so 200 machines.
        assert!(
            (process.machines() - 200.0).abs() < 1e-9,
            "{}",
            process.machines()
        );
    }

    #[test]
    fn a_hidden_recipe_is_inferred_but_not_suggested() {
        let registry = test_registry();
        let settings = Settings::default();
        // The recycler only makes hidden recipes, so inference has to see them.
        let document = ":iron-plate: into :recycler:";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected a process, got {:?}", outputs[0]);
        };
        let Value::Process(process) = &**value else {
            unreachable!()
        };
        assert_eq!(process.rows.len(), 1);
    }

    #[test]
    fn quality_modules_spread_the_output_across_qualities() {
        let registry = test_registry();
        let settings = Settings::default();
        let document =
            "p = 1 :foundry: :quality-module-2: making (:normal: :iron-plate:)\np.out.:iron-plate:";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[1] else {
            panic!("expected facets, got {:?}", outputs[1]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        let quantity = multi.quantity.clone().expect("a quantity");
        let super::value::Amount::Resources(terms) = &quantity.amount else {
            panic!("expected a bundle");
        };

        // A 25% chance upgrades a product by one level, and each further level
        // is a constant 10% roll: 0.9Q, 0.09Q, 0.009Q, then 0.001Q for the top
        // tier (https://wiki.factorio.com/Quality#Quality_modules).
        let iron = registry.entry_id("iron-plate").unwrap();
        let rate = |quality| {
            terms
                .get(&Resource::Item(Item { id: iron, quality }))
                .copied()
                .unwrap_or(0.0)
        };
        assert_eq!(terms.len(), 5, "every quality is represented");
        // 1.875 plates/s overall, split 0.225 / 0.0225 / 0.00225 / 0.00025.
        let total: f64 = terms.values().sum();
        assert!((total - 1.875).abs() < 1e-9, "{total}");
        assert!((rate(Quality::Uncommon) - 1.875 * 0.225).abs() < 1e-9);
        assert!((rate(Quality::Rare) - 1.875 * 0.0225).abs() < 1e-9);
        assert!((rate(Quality::Epic) - 1.875 * 0.00225).abs() < 1e-9);
        assert!((rate(Quality::Legendary) - 1.875 * 0.00025).abs() < 1e-9);
        // Normal keeps the remainder, so it is the largest share here.
        assert!(rate(Quality::Normal) > rate(Quality::Uncommon));
        assert!((rate(Quality::Normal) - 1.875 * 0.75).abs() < 1e-9);
    }

    #[test]
    fn recipe_expansion_only_includes_researched_qualities() {
        let registry = test_registry();
        let mut settings = Settings::default();
        let qualities = |settings: &Settings| {
            let outputs = evaluate(
                ":foundry: making :iron-plate:",
                &registry,
                settings,
                &|| false,
            )
            .outputs;
            let Output::Value(value) = &outputs[0] else {
                panic!("expected a process, got {:?}", outputs[0]);
            };
            let Value::Process(process) = &**value else {
                unreachable!()
            };
            process
                .rows
                .keys()
                .map(|key| key.recipe_quality)
                .collect::<Vec<_>>()
        };

        assert_eq!(qualities(&settings), Quality::ALL.to_vec());
        settings.max_quality = Quality::Rare;
        assert_eq!(
            qualities(&settings),
            vec![Quality::Normal, Quality::Uncommon, Quality::Rare]
        );
    }

    #[test]
    fn max_quality_folds_higher_quality_into_the_highest_researched() {
        let registry = test_registry();
        let settings = Settings {
            max_quality: Quality::Rare,
            ..Settings::default()
        };
        let document =
            "p = 1 :foundry: :quality-module-2: making (:normal: :iron-plate:)\np.out.:iron-plate:";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[1] else {
            panic!("expected facets, got {:?}", outputs[1]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        let quantity = multi.quantity.clone().expect("a quantity");
        let super::value::Amount::Resources(terms) = &quantity.amount else {
            panic!("expected a bundle");
        };

        let iron = registry.entry_id("iron-plate").unwrap();
        let rate = |quality| {
            terms
                .get(&Resource::Item(Item { id: iron, quality }))
                .copied()
                .unwrap_or(0.0)
        };
        // A 25% chance: normal 75%, uncommon 22.5%, and rare keeps everything
        // that would have rolled epic or legendary.
        assert_eq!(terms.len(), 3);
        let total: f64 = terms.values().sum();
        assert!((total - 1.875).abs() < 1e-9, "{total}");
        assert!((rate(Quality::Normal) - 1.875 * 0.75).abs() < 1e-9);
        assert!((rate(Quality::Uncommon) - 1.875 * 0.225).abs() < 1e-9);
        assert!((rate(Quality::Rare) - 1.875 * 0.025).abs() < 1e-9);
    }

    #[test]
    fn a_quality_loop_does_not_freeze_its_residuals() {
        // Each pass recycles everything below rare, so the normal and uncommon
        // rows shrink geometrically. Their nets pass through the approximate
        // threshold, and a net dropped there would strand the residue the loop
        // had yet to consume.
        let registry = test_registry();
        let settings = Settings {
            max_quality: Quality::Rare,
            ..Settings::default()
        };
        let document = "qm = :quality-module-2:\np = 1:foundry: making :casting-iron:\nloop 100 {\np += (p.net<:rare:) into :recycler:(1 qm)\n}\np.net";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[5] else {
            panic!("expected a process, got {:?}", outputs[5]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        let quantity = multi.quantity.as_ref().expect("a quantity");
        let Amount::Resources(terms) = &quantity.amount else {
            panic!("expected a bundle");
        };
        let rates: Vec<(Quality, f64)> = terms
            .iter()
            .filter_map(|(resource, amount)| match resource {
                Resource::Item(item) => Some((item.quality, *amount)),
                Resource::Fluid(_) => None,
            })
            .collect();
        assert_eq!(rates.len(), 1, "{rates:?}");
        assert_eq!(rates[0].0, Quality::Rare);
        assert!((rates[0].1 - 1.25).abs() < 1e-6, "{}", rates[0].1);
    }

    #[test]
    fn a_process_answers_in_out_and_net() {
        let registry = test_registry();
        let settings = Settings::default();
        let render = |text: &str| {
            let document = format!("p = 1 :foundry: making (:normal: :iron-plate:)\n{text}");
            let outputs = evaluate(&document, &registry, &settings, &|| false).outputs;
            match &outputs[1] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        let ore = registry
            .entry_by_index(registry.entry_id("iron-ore").unwrap())
            .glyph;
        let plate = registry
            .entry_by_index(registry.entry_id("iron-plate").unwrap())
            .glyph;

        // `in` is a keyword and a property; after a dot it is the property.
        // Inputs are positive there; only `net` carries the sign.
        assert_eq!(render("p.in.:iron-ore:"), format!("1.25{ore}/s"));
        assert_eq!(render("p.out.:iron-plate:"), format!("1.875{plate}/s"));
        assert_eq!(render("p.net.:iron-ore:"), format!("-1.25{ore}/s"));
        assert_eq!(render("p.net.:iron-plate:"), format!("1.875{plate}/s"));
    }

    #[test]
    fn a_process_answers_speed_when_its_rows_agree() {
        let registry = test_registry();
        let settings = Settings::default();
        let document = "p = 1 :foundry: making (:normal: :iron-plate:)\np.:speed:";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        match &outputs[1] {
            Output::Value(value) => assert_eq!(value.dimensionless(), Some(4.0)),
            other => panic!("expected a value, got {other:?}"),
        }
    }

    #[test]
    fn idle_power_follows_the_setting() {
        let registry = test_registry();
        let render = |text: &str, idle_power| {
            let settings = Settings {
                idle_power,
                ..Settings::default()
            };
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };

        // The foundry draws 2 MW active and a thirtieth more when idle.
        assert_eq!(render("(2 :foundry:).power", IdlePower::Ignored), "4MW");
        assert_eq!(
            render("(2 :foundry:).power", IdlePower::Fractional),
            "4.133MW"
        );
        assert_eq!(
            render("(2 :foundry:).power", IdlePower::RoundedUp),
            "4.133MW"
        );

        // A fractional count scales the active draw but not the whole machines
        // that idle draw is charged for.
        assert_eq!(
            render("(2.5 :foundry:).power", IdlePower::Fractional),
            "5.167MW"
        );
        assert_eq!(
            render("(2.5 :foundry:).power", IdlePower::RoundedUp),
            "5.2MW"
        );
        // A count within the approximate threshold is already whole.
        assert_eq!(
            render("(2.0000000001 :foundry:).power", IdlePower::RoundedUp),
            "4.133MW"
        );

        // The process reports the same sum as its rows.
        assert_eq!(
            render(
                "(2.5 :foundry: making (:normal: :iron-plate:)).power",
                IdlePower::RoundedUp
            ),
            "5.2MW"
        );
    }

    #[test]
    fn quality_improves_only_what_a_module_is_for() {
        let registry = test_registry();
        let settings = Settings::default();
        let number = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => value.dimensionless().expect("dimensionless"),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        let render = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };

        // A legendary productivity module gives 2.5 times the productivity and
        // leaves its +40% consumption alone: 2 MW * 1.4.
        assert_eq!(
            number("(:foundry: :legendary::productivity-module-1:).productivity"),
            1.6
        );
        assert_eq!(
            render("(:foundry: :legendary::productivity-module-1:).power"),
            "2.8MW"
        );

        // A legendary speed module is faster, and no hungrier: 2 MW * 1.7.
        assert_eq!(
            number("(:foundry: :legendary::speed-module-2:).:speed:"),
            7.0
        );
        assert_eq!(
            render("(:foundry: :legendary::speed-module-2:).power"),
            "3.4MW"
        );

        // A legendary efficiency module saves more, and changes nothing else:
        // -0.3 * 2.5 = -0.75 consumption, so 2 MW * 0.25, with base productivity.
        assert_eq!(
            render("(:foundry: :legendary::efficiency-module-1:).power"),
            "500kW"
        );
        assert_eq!(
            number("(:foundry: :legendary::efficiency-module-1:).productivity"),
            1.5
        );
    }

    #[test]
    fn a_process_selects_by_medium() {
        let registry = test_registry();
        let settings = Settings::default();
        let render = |text: &str| {
            let document = format!("p = 1 :foundry: making (:normal: :iron-plate:)\n{text}");
            let outputs = evaluate(&document, &registry, &settings, &|| false).outputs;
            match &outputs[1] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        let ore = registry
            .entry_by_index(registry.entry_id("iron-ore").unwrap())
            .glyph;
        let plate = registry
            .entry_by_index(registry.entry_id("iron-plate").unwrap())
            .glyph;

        // Items are both the ore consumed and the plates produced.
        assert_eq!(render("p.:item:"), format!("-1.25{ore}/s + 1.875{plate}/s"));
        assert_eq!(render("p.:fluid:"), "0");

        // A rare selection is empty, and quality renders as its own glyph. A
        // zero quantity carries no units (§5.3).
        assert_eq!(render("p.net.:rare: :iron-plate:"), "0");
        let rare = hactorio_data::quality_by_id("rare").unwrap().glyph;
        assert_eq!(crate::eval::Quality::Rare.glyph(), rare);
        let legendary = hactorio_data::quality_by_id("legendary").unwrap().glyph;
        assert_ne!(rare, legendary);
    }

    #[test]
    fn a_zero_amount_has_no_units_and_adds_to_anything() {
        let registry = test_registry();
        let settings = Settings::default();
        let render = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        let plate = registry
            .entry_by_index(registry.entry_id("iron-plate").unwrap())
            .glyph;

        // Zero is the additive identity whatever shape it was written in.
        assert_eq!(render(":iron-plate: + 0"), format!("1{plate}"));
        assert_eq!(render("0 + :iron-plate:"), format!("1{plate}"));
        assert_eq!(render("3 kJ + 0"), "3kJ");

        // A zero numerator erases every unit it was written with.
        assert_eq!(render(":iron-plate: - :iron-plate:"), "0");
        assert_eq!(render("0 :iron-plate:"), "0");
        assert_eq!(render("0/s"), "0");
        assert_eq!(render(":iron-plate: * 0"), "0");
    }

    #[test]
    fn better_quality_helps() {
        let registry = test_registry();
        let settings = Settings::default();
        let number = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => value.dimensionless().expect("dimensionless"),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        // A legendary foundry is 2.5 times as fast (speed 4 -> 10).
        assert_eq!(number("(:legendary: :foundry:) .:speed:"), 10.0);
        // A legendary speed module gives 2.5 times the effect: 4 * (1 + 0.75).
        assert_eq!(
            number("(:foundry: :legendary::speed-module-2:) .:speed:"),
            7.0
        );
        // A legendary beacon transmits 2.5 times as much, so its normal
        // module's 0.3 arrives as 0.3 * 1.5 * 2.5.
        assert_eq!(
            number("(:foundry: :legendary::beacon::speed-module-2:) .:speed:"),
            8.5
        );
    }

    #[test]
    fn a_machine_rejects_more_modules_than_it_has_slots() {
        let registry = test_registry();
        let settings = Settings::default();
        let outputs = evaluate(
            ":foundry: 5 :speed-module-2:",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        assert!(
            matches!(outputs[0], Output::Error { .. }),
            "five modules should not fit four slots, got {:?}",
            outputs[0]
        );
        // Four fit exactly.
        let outputs = evaluate(
            ":foundry: 4 :speed-module-2:",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        assert!(matches!(outputs[0], Output::Value(_)));
    }

    #[test]
    fn quality_aliases_resolve_as_tokens() {
        let registry = test_registry();
        let settings = Settings::default();
        let level = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => value.dimensionless().expect("dimensionless"),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        for (token, expected) in [
            (":common:", 0.0),
            (":quality-1:", 0.0),
            (":quality-2:", 1.0),
            (":quality-3:", 2.0),
            (":quality-4:", 3.0),
            (":quality-5:", 5.0),
        ] {
            assert_eq!(level(&format!("{token} .level")), expected, "{token}");
        }
        // The aliases qualify a token exactly like the canonical name, so both
        // render without a quality glyph.
        let rendered = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            match &outputs[0] {
                Output::Value(value) => format_value(value, &settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        assert_eq!(
            rendered(":common::iron-plate:"),
            rendered(":normal::iron-plate:")
        );
        assert_eq!(
            rendered(":quality-5::iron-plate:"),
            rendered(":legendary::iron-plate:")
        );
    }

    #[test]
    fn a_dot_takes_any_expression_naming_a_resource() {
        let registry = test_registry();
        let settings = Settings::default();
        let plate = registry
            .entry_by_index(registry.entry_id("iron-plate").unwrap())
            .glyph;

        let document = "p = 1 :foundry: making (:normal: :iron-plate:)\nq = :iron-plate:\np . q";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[2] else {
            panic!("expected a value, got {:?}", outputs[2]);
        };
        assert_eq!(
            format_value(value, &settings, &registry),
            format!("1.875{plate}/s")
        );

        // The same selection, spelled with parentheses and a quality.
        let document =
            "p = 1 :foundry: making (:normal: :iron-plate:)\np . (:normal: :iron-plate:)";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[1] else {
            panic!("expected a value, got {:?}", outputs[1]);
        };
        assert_eq!(
            format_value(value, &settings, &registry),
            format!("1.875{plate}/s")
        );

        // A selector that names nothing is an error, not a silent zero.
        let document = "p = 1 :foundry: making (:normal: :iron-plate:)\np . 3";
        let outputs = evaluate(document, &registry, &settings, &|| false).outputs;
        assert!(matches!(outputs[1], Output::Error { .. }));
    }

    #[test]
    fn the_panel_shows_aggregate_rows() {
        let registry = test_registry();
        let settings = Settings::default();
        let panel = |text: &str| {
            let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
            let Output::Value(value) = &outputs[0] else {
                panic!("expected a process, got {:?}", outputs[0]);
            };
            let Value::Process(process) = &**value else {
                unreachable!()
            };
            format_process_panel(process, &settings, &registry)
        };
        let glyph = |id: &str| {
            registry
                .entry_by_index(registry.entry_id(id).unwrap())
                .glyph
        };
        let (foundry, ore, plate) = (glyph("foundry"), glyph("iron-ore"), glyph("iron-plate"));

        let lines = panel("540 :iron-plate:/s from (:foundry: making (:normal: :iron-plate:))");
        println!("{lines:#?}");
        assert_eq!(
            lines,
            vec![
                "power 576MW".to_string(),
                "pollution 0/m".to_string(),
                format!("288{foundry} · 360 craft/s"),
                "in".to_string(),
                format!("360{ore}/s"),
                "out".to_string(),
                format!("540{plate}/s"),
            ]
        );

        // A machine-seconds process reports totals and machine-seconds.
        let lines = panel("100 :iron-plate: from (:foundry: making (:normal: :iron-plate:))");
        println!("{lines:#?}");
        assert_eq!(
            lines,
            vec![
                "energy 106.667MJ".to_string(),
                "pollution 0".to_string(),
                format!("53.333s*{foundry} · 66.667 craft"),
                "in".to_string(),
                format!("66.667{ore}"),
                "out".to_string(),
                format!("100{plate}"),
            ]
        );
    }

    #[test]
    fn process_rows_are_assignable() {
        let registry = test_registry();
        let settings = Settings::default();
        let plate = registry
            .entry_by_index(registry.entry_id("iron-plate").unwrap())
            .glyph;

        // Two foundries make 3.75 plates/s; adding ten more is a row write.
        let mut document = String::from("p = 2 :foundry: making (:normal: :iron-plate:)\n");
        document.push_str("p.:iron-plate: += 10 :iron-plate:/s\n");
        document.push_str("p.net.:iron-plate:");
        let outputs = evaluate(&document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[2] else {
            panic!("expected a value, got {:?}", outputs[2]);
        };
        assert_eq!(
            format_value(value, &settings, &registry),
            format!("13.75{plate}/s")
        );

        // `=` sets the row's total, so the stored difference is 5 - 1.875.
        let mut document = String::from("p = 1 :foundry: making (:normal: :iron-plate:)\n");
        document.push_str("p.:iron-plate: = 5 :iron-plate:/s\n");
        document.push_str("p.net.:iron-plate:");
        let outputs = evaluate(&document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[2] else {
            panic!("expected a value, got {:?}", outputs[2]);
        };
        assert_eq!(
            format_value(value, &settings, &registry),
            format!("5{plate}/s")
        );

        // An unspecified quality means normal, so the same row is written.
        let mut document = String::from("p = 1 :foundry: making (:normal: :iron-plate:)\n");
        document.push_str("p.:iron-plate: += 5 :iron-plate:/s\n");
        document.push_str("p.net.:iron-plate:");
        let outputs = evaluate(&document, &registry, &settings, &|| false).outputs;
        let Output::Value(value) = &outputs[2] else {
            panic!("expected a value, got {:?}", outputs[2]);
        };
        assert_eq!(
            format_value(value, &settings, &registry),
            format!("6.875{plate}/s")
        );

        // Only resources are assignable.
        for refused in ["p.crafts = 1", "p.machines = 1", "p.power = 1"] {
            let document = format!("p = 1 :foundry: making (:normal: :iron-plate:)\n{refused}");
            let outputs = evaluate(&document, &registry, &settings, &|| false).outputs;
            assert!(
                matches!(outputs[1], Output::Error { .. }),
                "`{refused}` should be refused, got {:?}",
                outputs[1]
            );
        }
    }

    #[test]
    fn recycling_resolves_the_hidden_recipe() {
        let registry = test_registry();
        let outputs = evaluate(
            ":recycling: :iron-plate:",
            &registry,
            &Settings::default(),
            &|| false,
        )
        .outputs;
        let Output::Value(value) = &outputs[0] else {
            panic!("expected facets, got {:?}", outputs[0]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        assert!(multi.recipe.is_some());
    }

    fn facets_of(text: &str) -> Vec<crate::eval::Output> {
        let registry = test_registry();
        evaluate(text, &registry, &Settings::default(), &|| false).outputs
    }

    fn error_of(outputs: &[crate::eval::Output], index: usize) -> (String, super::parse::Span) {
        match &outputs[index] {
            crate::eval::Output::Error { message, span } => (message.clone(), *span),
            other => panic!("expected an error, got {other:?}"),
        }
    }

    #[test]
    fn k_is_a_variable_name() {
        // `k` is a number suffix, not a reserved word (§4.4).
        let registry = test_registry();
        let settings = Settings::default();
        let outputs = evaluate("k = 1\nk", &registry, &settings, &|| false).outputs;
        assert_eq!(
            outputs[1],
            crate::eval::Output::Value(Box::new(Value::number(1.0)))
        );
        let outputs = evaluate("k = 2\n1k", &registry, &settings, &|| false).outputs;
        assert_eq!(
            outputs[1],
            crate::eval::Output::Value(Box::new(Value::number(1000.0)))
        );
    }

    #[test]
    fn negation_drops_the_reference_facets() {
        let outputs = facets_of("-:iron-plate:");
        let crate::eval::Output::Value(value) = &outputs[0] else {
            panic!("expected facets, got {:?}", outputs[0]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        assert!(multi.quantity.is_some());
        assert!(multi.resource.is_none(), "resource: {:?}", multi.resource);
    }

    #[test]
    fn abs_drops_the_config() {
        let outputs = facets_of("(2 :speed-module-2:).abs");
        let crate::eval::Output::Value(value) = &outputs[0] else {
            panic!("expected facets, got {:?}", outputs[0]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        assert!(multi.config.is_none(), "config: {:?}", multi.config);
    }

    #[test]
    fn a_bundle_accepts_the_representable_exponents() {
        let plate = registry_glyph("iron-plate");
        assert_eq!(render("(2:iron-plate:)^1"), format!("2{plate}"));
        assert_eq!(render("(2:iron-plate:)^0"), "1");
        assert_eq!(render("(2:iron-plate:)^-1"), format!("0.5/{plate}"));
        // A second item term would make the reciprocal unrepresentable.
        assert!(matches!(
            facets_of("(2:iron-plate: + 1:copper-plate:)^-1")[0],
            crate::eval::Output::Error { .. }
        ));
        assert!(matches!(
            facets_of("(2:iron-plate:)^2")[0],
            crate::eval::Output::Error { .. }
        ));
    }

    #[test]
    fn an_unscaled_process_cannot_be_timed() {
        assert!(matches!(
            facets_of("1 m * (:foundry: making (:normal: :iron-plate:))")[0],
            crate::eval::Output::Error { .. }
        ));
    }

    #[test]
    fn a_beacon_profile_must_cover_the_beacon_count() {
        // The test registry's profile has eight entries, so nine beacons has no
        // distribution entry (§6.2).
        let outputs =
            facets_of("1 :foundry: (9 :beacon: :speed-module-2:) making (:normal: :iron-plate:)");
        let (message, _) = error_of(&outputs, 0);
        assert!(message.contains("beacon profile"), "{message}");
    }

    #[test]
    fn a_zero_factor_does_not_scale_a_config() {
        let outputs = facets_of("0 (2 :speed-module-2:)");
        let crate::eval::Output::Value(value) = &outputs[0] else {
            panic!("expected facets, got {:?}", outputs[0]);
        };
        let Value::Multi(multi) = &**value else {
            unreachable!()
        };
        assert!(multi.config.is_none(), "config: {:?}", multi.config);
    }

    #[test]
    fn the_panel_uses_the_configured_time_unit_for_crafts() {
        let registry = test_registry();
        let settings = Settings {
            time_unit: crate::TimeUnit::Minute,
            ..Settings::default()
        };
        let outputs = evaluate(
            "p = 1 :foundry: making (:normal: :iron-plate:)\np",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        let crate::eval::Output::Value(value) = &outputs[1] else {
            panic!("expected a process, got {:?}", outputs[1]);
        };
        let panel = format_facets(value, &settings, &registry).join("\n");
        assert!(panel.contains("craft/m"), "{panel}");
        assert!(!panel.contains("craft/s"), "{panel}");
    }

    #[test]
    fn a_poisoned_read_names_the_definition_line() {
        let outputs = facets_of("x = 1:iron-plate: + 1s\ny = x");
        let (message, span) = error_of(&outputs, 1);
        assert_eq!(message, "`x` has error from line 1");
        assert_eq!(span, crate::eval::parse::Span::new(4, 5));
    }

    #[test]
    fn a_loop_aborts_after_the_failing_iteration() {
        let outputs = facets_of("loop 5 {\n  x = 1:iron-plate: + 1s\n}\n1");
        let (message, _) = error_of(&outputs, 0);
        assert_eq!(
            message,
            "loop aborted after 1 iteration due to error on line 2"
        );
        // The statement after the loop still runs (§2.5).
        assert!(matches!(outputs[3], crate::eval::Output::Value(_)));
    }

    #[test]
    fn belt_mode_renders_item_rates_as_belt_counts() {
        let registry = test_registry();
        let belt = registry
            .entry_by_index(registry.entry_id("transport-belt").unwrap())
            .glyph;
        let plate = registry
            .entry_by_index(registry.entry_id("iron-plate").unwrap())
            .glyph;
        let mut settings = Settings {
            item_rate_belt_display: Some(crate::belt::BeltTier::Yellow),
            ..Settings::default()
        };
        let render = |settings: &Settings| {
            let outputs = evaluate("15 :iron-plate:/s", &registry, settings, &|| false).outputs;
            match &outputs[0] {
                crate::eval::Output::Value(value) => format_value(value, settings, &registry),
                other => panic!("expected a value, got {other:?}"),
            }
        };
        assert_eq!(render(&settings), format!("1 {belt} {plate}"));
        settings.stacked_belts_display = true;
        let stacked = render(&settings);
        assert!(stacked.contains(STACK_SIZE_GLYPH), "{stacked}");
        assert!(stacked.contains(belt), "{stacked}");
    }

    #[test]
    fn an_eval_error_underlines_the_failing_subexpression() {
        let line = "x = 2 * (1:iron-plate: + 1s)";
        let outputs = facets_of(line);
        let (_, span) = error_of(&outputs, 0);
        assert_eq!(&line[span.start..span.end], "1:iron-plate: + 1s");
    }

    #[test]
    fn timing_names_the_extent_that_cannot_be_timed() {
        // An unscaled process is the problem, not machine-seconds.
        let outputs = facets_of("1m * (:foundry: making (:normal: :iron-plate:))");
        let (message, _) = error_of(&outputs, 0);
        assert!(message.contains("unscaled"), "{message}");
        // A machine-seconds process cannot be timed twice.
        let outputs = facets_of("p = 1 :foundry: making (:normal: :iron-plate:)\np * 1s * 1s");
        let (message, _) = error_of(&outputs, 1);
        assert!(message.contains("machine-seconds"), "{message}");
    }

    fn extent_of(outputs: &[crate::eval::Output], index: usize) -> crate::eval::value::Extent {
        let crate::eval::Output::Value(value) = &outputs[index] else {
            panic!("expected a process, got {:?}", outputs[index]);
        };
        let Value::Process(process) = &**value else {
            unreachable!()
        };
        process.extent
    }

    #[test]
    fn compound_assignment_needs_the_rows_unit_and_item() {
        let registry = test_registry();
        let settings = Settings::default();
        let run = |line: &str| {
            let document = format!("p = 1 :foundry: making (:normal: :iron-plate:)\n{line}");
            evaluate(&document, &registry, &settings, &|| false).outputs
        };

        // A machines row takes a rate, with no item or the row's own item at
        // normal quality.
        for line in [
            "p.:iron-plate: += 5/s",
            "p.:iron-plate: += 5 :iron-plate:/s",
            "p.:iron-plate: += 5 :normal: :iron-plate:/s",
            "p.:iron-plate: += :transport-belt: :iron-plate:",
        ] {
            let outputs = run(line);
            assert!(
                matches!(outputs[1], crate::eval::Output::Value(_)),
                "{line}: {:?}",
                outputs[1]
            );
        }

        for line in [
            "p.:iron-plate: += 5 :iron-plate:",
            "p.:iron-plate: += 5 :rare: :iron-plate:/s",
            "p.:iron-plate: += 5",
            "p.:iron-plate: += :transport-belt:",
        ] {
            let outputs = run(line);
            assert!(
                matches!(outputs[1], crate::eval::Output::Error { .. }),
                "{line}: {:?}",
                outputs[1]
            );
        }
    }

    #[test]
    fn a_machine_seconds_row_takes_an_amount() {
        let registry = test_registry();
        let settings = Settings::default();
        let run = |line: &str| {
            let document = format!(
                "p = 100 :iron-ore: into (:foundry: making (:normal: :iron-plate:))\n{line}"
            );
            evaluate(&document, &registry, &settings, &|| false).outputs
        };
        for line in ["p.:iron-plate: += 5", "p.:iron-plate: += 5 :iron-plate:"] {
            let outputs = run(line);
            assert!(
                matches!(outputs[1], crate::eval::Output::Value(_)),
                "{line}: {:?}",
                outputs[1]
            );
        }
        for line in [
            "p.:iron-plate: += 5/s",
            "p.:iron-plate: += :transport-belt:",
            "p.:iron-plate: += :transport-belt: :iron-plate:",
        ] {
            let outputs = run(line);
            assert!(
                matches!(outputs[1], crate::eval::Output::Error { .. }),
                "{line}: {:?}",
                outputs[1]
            );
        }
    }

    #[test]
    fn an_unscaled_process_cannot_be_assigned() {
        let registry = test_registry();
        let settings = Settings::default();
        let outputs = evaluate(
            "p = :foundry: making (:normal: :iron-plate:)\np.:iron-plate: += 5/s",
            &registry,
            &settings,
            &|| false,
        )
        .outputs;
        let (message, _) = error_of(&outputs, 1);
        assert!(message.contains("unscaled"), "{message}");
    }

    #[test]
    fn assigning_an_unspecified_process_sizes_it() {
        let registry = test_registry();
        let settings = Settings::default();
        let run = |line: &str| {
            let document = format!(
                "z = 0 :iron-plate: into (:foundry: making (:normal: :iron-plate:))\n{line}"
            );
            evaluate(&document, &registry, &settings, &|| false).outputs
        };
        assert_eq!(
            extent_of(&run("z.:iron-plate: += 5/s"), 1),
            crate::eval::value::Extent::Machines
        );
        assert_eq!(
            extent_of(&run("z.:iron-plate: += 5"), 1),
            crate::eval::value::Extent::MachineSeconds
        );
        assert!(matches!(
            run("z.:iron-plate: += :transport-belt:")[1],
            crate::eval::Output::Error { .. }
        ));
    }

    fn registry_glyph(id: &str) -> String {
        let registry = test_registry();
        registry
            .entry_by_index(registry.entry_id(id).unwrap())
            .glyph
            .to_string()
    }

    fn render(text: &str) -> String {
        let registry = test_registry();
        let settings = Settings::default();
        let outputs = evaluate(text, &registry, &settings, &|| false).outputs;
        match &outputs[0] {
            crate::eval::Output::Value(value) => format_value(value, &settings, &registry),
            other => panic!("expected a value, got {other:?}"),
        }
    }
}
