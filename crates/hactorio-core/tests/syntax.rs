use hactorio_core::{
    BaseUnit, BeltTier, BinaryOp, Expr, ParseError, Span, Statement, StatementKind, Value,
    from_plaintext, parse_line, to_plaintext,
};
use hactorio_data::{Locale, Registry, postprocess};

const DUMP: &str = r#"{
  "item": {
    "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100, "icon": "a.png"},
    "iron-ore": {"type": "item", "name": "iron-ore", "stack_size": 50, "icon": "b.png"},
    "assembling-machine-3": {"type": "item", "name": "assembling-machine-3", "stack_size": 50, "icon": "c.png"}
  },
  "fluid": {
    "water": {"type": "fluid", "name": "water", "icon": "d.png"}
  },
  "module": {
    "speed-module-3": {
      "type": "module",
      "name": "speed-module-3",
      "category": "speed",
      "tier": 3,
      "stack_size": 50,
      "effect": {"speed": 0.5}
    }
  },
  "recipe": {
    "iron-plate": {
      "type": "recipe",
      "name": "iron-plate",
      "categories": ["smelting"],
      "energy_required": 3.2,
      "ingredients": [{"type": "item", "name": "iron-ore", "amount": 1}],
      "results": [{"type": "item", "name": "iron-plate", "amount": 1}]
    },
    "advanced-circuit": {
      "type": "recipe",
      "name": "advanced-circuit",
      "results": [{"type": "item", "name": "iron-plate", "amount": 1}]
    }
  },
  "assembling-machine": {
    "assembling-machine-3": {
      "type": "assembling-machine",
      "name": "assembling-machine-3",
      "crafting_categories": ["crafting"],
      "crafting_speed": 1.25,
      "module_slots": 4,
      "allowed_effects": ["speed"],
      "energy_usage": "375kW"
    }
  }
}"#;

fn registry() -> Registry {
    postprocess(DUMP, &Locale::empty()).unwrap()
}

fn parse(line: &str) -> Result<Statement, ParseError> {
    parse_line(line, &registry())
}

fn expr(line: &str) -> Expr {
    match parse(line) {
        Ok(Statement {
            kind: StatementKind::Expression(value),
            ..
        }) => value,
        other => panic!("expected an expression for {line:?}, got {other:?}"),
    }
}

fn literal(line: &str) -> Value {
    match expr(line) {
        Expr::Literal(value) => value,
        other => panic!("expected a literal for {line:?}, got {other:?}"),
    }
}

fn lit(value: f64) -> Expr {
    Expr::Literal(Value::scalar(value))
}

fn binary(op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

#[test]
fn parses_decimal_numbers_and_metric_suffixes() {
    assert_eq!(literal("3"), Value::scalar(3.0));
    assert_eq!(literal("3."), Value::scalar(3.0));
    assert_eq!(literal(".5"), Value::scalar(0.5));
    assert_eq!(literal("1.25"), Value::scalar(1.25));
    assert_eq!(literal("3k"), Value::scalar(3000.0));
    assert_eq!(literal("1.5M"), Value::scalar(1_500_000.0));
    assert_eq!(literal("250G"), Value::scalar(250e9));
}

#[test]
fn rejects_non_decimal_numbers() {
    for line in ["3 k", "1e3", "1_000", "3 kk"] {
        assert!(parse(line).is_err(), "{line:?} should not parse");
    }
}

#[test]
fn parses_plaintext_and_glyph_tokens_identically() {
    let registry = registry();
    let expected = Expr::Literal(Value::new(
        1.0,
        [(BaseUnit::Item(registry.entry_id("iron-plate").unwrap()), 1)],
    ));

    assert_eq!(expr(":iron-plate:"), expected);
    let glyph = registry.entry("iron-plate").unwrap().glyph.to_string();
    assert_eq!(expr(&glyph), expected);

    assert_eq!(
        literal(":water:"),
        Value::new(
            1.0,
            [(BaseUnit::Fluid(registry.entry_id("water").unwrap()), 1)]
        )
    );
}

#[test]
fn rejects_unknown_prototypes() {
    for line in [":nope:", ":iron-plate", "::", ":"] {
        assert!(parse(line).is_err(), "{line:?} should not parse");
    }
}

#[test]
fn parses_time_and_unit_aliases() {
    assert_eq!(literal("3 m"), Value::minutes(3.0));
    assert_eq!(literal("2 hour"), Value::hours(2.0));
    assert_eq!(literal("s"), Value::seconds(1.0));
    assert_eq!(literal("3 craft"), Value::crafts(3.0));
    assert_eq!(literal("2 watt"), Value::watts(2.0));
    assert_eq!(literal("5 joule"), Value::joules(5.0));
    assert_eq!(literal("craft"), Value::crafts(1.0));
}

#[test]
fn unary_minus_binds_looser_than_power() {
    // -2^2 == -(2^2)
    assert_eq!(
        expr("-2^2"),
        Expr::Neg(Box::new(binary(BinaryOp::Pow, lit(2.0), lit(2.0))))
    );
    // 2^-2 keeps the minus inside the exponent.
    assert_eq!(
        expr("2^-2"),
        binary(BinaryOp::Pow, lit(2.0), Expr::Neg(Box::new(lit(2.0))))
    );
    // ^ is right-associative.
    assert_eq!(
        expr("2^3^2"),
        binary(
            BinaryOp::Pow,
            lit(2.0),
            binary(BinaryOp::Pow, lit(3.0), lit(2.0))
        )
    );
    // Unary minus binds tighter than multiplication.
    assert_eq!(
        expr("-2*3"),
        binary(BinaryOp::Mul, Expr::Neg(Box::new(lit(2.0))), lit(3.0))
    );
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    assert_eq!(
        expr("1+2*3"),
        binary(
            BinaryOp::Add,
            lit(1.0),
            binary(BinaryOp::Mul, lit(2.0), lit(3.0))
        )
    );
    assert_eq!(
        expr("(1+2)*3"),
        binary(
            BinaryOp::Mul,
            binary(BinaryOp::Add, lit(1.0), lit(2.0)),
            lit(3.0)
        )
    );
}

#[test]
fn parses_definitions() {
    let registry = registry();
    let plate = registry.entry_id("iron-plate").unwrap();
    match parse("rate = 3 :iron-plate: / second").unwrap().kind {
        StatementKind::Definition { name, value } => {
            assert_eq!(name, "rate");
            assert_eq!(
                value,
                binary(
                    BinaryOp::Div,
                    Expr::Literal(Value::new(3.0, [(BaseUnit::Item(plate), 1)])),
                    Expr::Literal(Value::seconds(1.0)),
                )
            );
        }
        other => panic!("expected a definition, got {other:?}"),
    }
}

#[test]
fn rejects_reserved_and_prototype_names() {
    let reserved = parse("in = 3").unwrap_err();
    assert!(reserved.message.contains("variable name"), "{reserved:?}");

    let collision = parse("water = 3").unwrap_err();
    assert!(collision.message.contains("prototype id"), "{collision:?}");

    assert!(parse("3 = 4").is_err());
}

#[test]
fn parses_machine_specs() {
    let registry = registry();
    let spec = match expr("3 @:assembling-machine-3: 2 :speed-module-3: making :iron-plate:") {
        Expr::Machine(spec) => spec,
        other => panic!("expected a machine, got {other:?}"),
    };

    assert_eq!(
        spec.machine,
        registry.entry_id("assembling-machine-3").unwrap()
    );
    assert_eq!(spec.count, 3.0);
    assert_eq!(spec.modules.len(), 1);
    assert_eq!(
        spec.modules[0].module,
        registry.entry_id("speed-module-3").unwrap()
    );
    assert_eq!(spec.modules[0].count, 2.0);
    assert_eq!(
        spec.recipe,
        registry
            .entry_id("iron-plate")
            .and_then(|id| registry.entry_by_index(id).recipe)
    );
}

#[test]
fn machine_defaults_are_one_machine_and_one_module() {
    let spec = match expr("@:assembling-machine-3: :speed-module-3:") {
        Expr::Machine(spec) => spec,
        other => panic!("expected a machine, got {other:?}"),
    };
    assert_eq!(spec.count, 1.0);
    assert_eq!(spec.modules[0].count, 1.0);
    assert!(spec.recipe.is_none());
}

#[test]
fn rejects_invalid_machine_specs() {
    for line in [
        "@:iron-plate:",
        "@:speed-module-3:",
        "@:water:",
        "@:assembling-machine-3: :iron-plate:",
        "1.5 @:assembling-machine-3:",
        "0 @:assembling-machine-3:",
        "@:assembling-machine-3: 0 :speed-module-3:",
        "@:assembling-machine-3: making :water:",
        "@:assembling-machine-3: making",
        "@",
    ] {
        assert!(parse(line).is_err(), "{line:?} should not parse");
    }
}

#[test]
fn parses_belt_literals() {
    let registry = registry();
    let plate = registry.entry_id("iron-plate").unwrap();

    assert_eq!(
        expr("2 yb :iron-plate:"),
        Expr::Belt {
            count: 2.0,
            tier: BeltTier::Yellow,
            stacked: false,
            item: plate
        }
    );
    assert_eq!(
        expr("sgb :iron-plate:"),
        Expr::Belt {
            count: 1.0,
            tier: BeltTier::Green,
            stacked: true,
            item: plate
        }
    );
}

#[test]
fn belt_literals_require_a_positive_count_and_an_item() {
    for line in [
        "0 yb :iron-plate:",
        "yb :water:",
        "yb :advanced-circuit:",
        "yb 3",
    ] {
        assert!(parse(line).is_err(), "{line:?} should not parse");
    }
}

#[test]
fn parses_in_expressions() {
    let registry = registry();
    let machine = match expr("100 :iron-plate: / second in @:assembling-machine-3:") {
        Expr::In { quantity, machine } => {
            assert!(matches!(
                *quantity,
                Expr::Binary {
                    op: BinaryOp::Div,
                    ..
                }
            ));
            match *machine {
                Expr::Machine(spec) => spec,
                other => panic!("expected a machine spec, got {other:?}"),
            }
        }
        other => panic!("expected an in expression, got {other:?}"),
    };
    assert_eq!(
        machine.machine,
        registry.entry_id("assembling-machine-3").unwrap()
    );
    assert_eq!(machine.count, 1.0);

    // Q in M * 2 is (Q in M) * 2, because the right operand ends at `*`.
    assert!(matches!(
        expr("Q in @:assembling-machine-3: * 2"),
        Expr::Binary {
            op: BinaryOp::Mul,
            ..
        }
    ));

    // a + b in M is (a + b) in M.
    match expr("Q + R in @:assembling-machine-3:") {
        Expr::In { quantity, .. } => {
            assert!(matches!(
                *quantity,
                Expr::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            ))
        }
        other => panic!("expected an in expression, got {other:?}"),
    }

    // A machine spec can carry its own count.
    match expr("Q in 3 @:assembling-machine-3:") {
        Expr::In { machine, .. } => match *machine {
            Expr::Machine(spec) => assert_eq!(spec.count, 3.0),
            other => panic!("expected a machine spec, got {other:?}"),
        },
        other => panic!("expected an in expression, got {other:?}"),
    }

    // Variables and parenthesized machine values are allowed on the right.
    assert!(matches!(
        expr("Q in furnace"),
        Expr::In { machine, .. } if matches!(*machine, Expr::Variable(_))
    ));
    assert!(matches!(
        expr("Q in (2 * @:assembling-machine-3:)"),
        Expr::In { machine, .. } if matches!(*machine, Expr::Binary { .. })
    ));
}

#[test]
fn in_is_not_associative() {
    let error = parse("Q in @:assembling-machine-3: in @:assembling-machine-3:").unwrap_err();
    assert!(error.message.contains("associative"), "{error:?}");
}

#[test]
fn recipe_and_module_tokens_are_not_operands() {
    let recipe = parse(":advanced-circuit:").unwrap_err();
    assert!(recipe.message.contains("recipe"), "{recipe:?}");

    let module = parse(":speed-module-3:").unwrap_err();
    assert!(module.message.contains("module"), "{module:?}");
}

#[test]
fn errors_point_at_the_offending_token() {
    let line = "3 @:iron-plate:";
    let start = line.find(":iron-plate:").unwrap() as u32;
    let error = parse(line).unwrap_err();
    assert_eq!(
        error.span,
        Span::new(start, start + ":iron-plate:".len() as u32)
    );
}

#[test]
fn blank_and_comment_lines_have_no_tokens() {
    assert_eq!(parse("").unwrap().kind, StatementKind::Blank);
    assert_eq!(parse("   ").unwrap().kind, StatementKind::Blank);
    assert_eq!(parse("# title").unwrap().kind, StatementKind::Comment);
    assert_eq!(parse("  # title").unwrap().kind, StatementKind::Comment);
}

#[test]
fn comments_end_a_statement() {
    assert_eq!(
        expr("3 + 4 # note"),
        binary(BinaryOp::Add, lit(3.0), lit(4.0))
    );
}

#[test]
fn trailing_tokens_are_rejected() {
    assert!(parse("3 4").is_err());
    assert!(parse("(3) 4").is_err());
    assert!(parse("3 @:assembling-machine-3: @:assembling-machine-3:").is_err());
}

#[test]
fn text_round_trips_between_plaintext_and_glyphs() {
    let registry = registry();
    let plain = "3 :iron-plate: / second";
    let glyphs = from_plaintext(plain, &registry);

    assert_eq!(
        glyphs.chars().count(),
        plain.chars().count() - ":iron-plate:".chars().count() + 1
    );
    assert_eq!(to_plaintext(&glyphs, &registry), plain);

    // Unknown tokens stay readable.
    assert_eq!(from_plaintext(":nope:", &registry), ":nope:");
    assert_eq!(to_plaintext(":nope:", &registry), ":nope:");
}

#[test]
fn number_before_a_prototype_is_a_quantity() {
    let registry = registry();
    let plate = registry.entry_id("iron-plate").unwrap();

    assert_eq!(
        literal("3 :iron-plate:"),
        Value::new(3.0, [(BaseUnit::Item(plate), 1)])
    );
    assert_eq!(
        expr("3 :iron-plate: / second"),
        binary(
            BinaryOp::Div,
            Expr::Literal(Value::new(3.0, [(BaseUnit::Item(plate), 1)])),
            Expr::Literal(Value::seconds(1.0)),
        )
    );

    for line in ["3 :advanced-circuit:", "3 :speed-module-3:"] {
        assert!(parse(line).is_err(), "{line:?} should not parse");
    }
}
