use super::*;
use egui::{Key, Modifiers};

fn suggested_ids(registry: &Registry, text: &str) -> Vec<String> {
    let cursor = text.chars().count();
    let (start, word) = completion_word(text, cursor);
    completion_items(registry, &Settings::default(), text, start, &word)
        .into_iter()
        .map(|item| item.insert)
        .collect()
}

#[test]
fn clicking_a_completion_item_accepts_it() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("click-accept", document);
    harness.run();
    type_into_editor(&mut harness, " i");

    let glyph = harness
        .state()
        .as_ref()
        .expect("app")
        .completion
        .items
        .first()
        .expect("completions")
        .insert
        .clone();
    let pos = harness
        .state()
        .as_ref()
        .expect("app")
        .completion
        .selected_rect
        .expect("selected row")
        .center();
    harness.hover_at(pos);
    for pressed in [true, false] {
        harness.input_mut().events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run();

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, format!("# Main {glyph}\n"));
    assert!(app.completion.items.is_empty());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn completion_scrolls_to_the_selected_item() {
    let items: Vec<String> = (0..40)
        .map(|index| {
            format!(
                r#""test-item-{index:02}": {{"type": "item", "name": "test-item-{index:02}", "stack_size": 1}}"#
            )
        })
        .collect();
    let fixture = format!(r#"{{"item": {{{}}}}}"#, items.join(","));
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness_with("scroll", document, &fixture);
    harness.run();
    type_into_editor(&mut harness, " i");
    // The 40 test items plus keywords that contain `i`.
    assert!(
        harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .len()
            >= 40
    );

    for _ in 0..20 {
        harness.key_press(Key::ArrowDown);
        harness.run();
    }
    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.completion.selected, 20);
    let selected = app.completion.selected_rect.expect("selected row");
    let popup = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::unique(("completion", 0usize))))
        .expect("popup");
    assert!(popup.contains_rect(selected), "{popup:?} {selected:?}");

    // Reopening after being closed scrolls back to the new selection.
    harness.key_press(Key::Escape);
    harness.run();
    assert!(
        harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .is_empty()
    );
    type_into_editor(&mut harness, "t");
    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.completion.selected, 0);
    let selected = app.completion.selected_rect.expect("selected row");
    let popup = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::unique(("completion", 0usize))))
        .expect("popup");
    assert!(popup.contains_rect(selected), "{popup:?} {selected:?}");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn completion_matches_subsequences_case_insensitively() {
    let registry = fixture_registry();
    let inserts = suggested_ids(&registry, ":IPL");
    let iron = registry.entry("iron-plate").unwrap().glyph.to_string();
    assert!(inserts.contains(&iron), "{inserts:?}");
}

#[test]
fn the_more_referenced_symbol_wins_a_close_match() {
    // `ice-platform` matches `ipl` slightly better than `iron-plate`, but iron
    // plates feed many more recipes, which should put them first.
    let sinks: Vec<String> = (0..30)
        .map(|index| {
            format!(
                r#""sink-{index}": {{"type": "recipe", "name": "sink-{index}", "ingredients": [{{"type": "item", "name": "iron-plate", "amount": 1}}], "results": [{{"type": "item", "name": "sink-{index}", "amount": 1}}]}}"#
            )
        })
        .collect();
    let fixture = format!(
        r#"{{"item": {{
            "iron-plate": {{"type": "item", "name": "iron-plate", "stack_size": 100}},
            "ice-platform": {{"type": "item", "name": "ice-platform", "stack_size": 50}}
        }}, "recipe": {{{}}}}}"#,
        sinks.join(",")
    );
    let registry = hactorio_data::postprocess(&fixture, &hactorio_data::Locale::empty()).unwrap();
    let iron = registry.entry("iron-plate").unwrap();
    let ice = registry.entry("ice-platform").unwrap();
    assert!(
        best_name_match(ice, "ipl").unwrap().0 > best_name_match(iron, "ipl").unwrap().0,
        "the fixture must reproduce the score reversal"
    );
    assert_eq!(
        registry.reference_count(registry.entry_id("iron-plate").unwrap()),
        30
    );

    let inserts = suggested_ids(&registry, "ipl");
    assert_eq!(
        inserts.first(),
        Some(&iron.glyph.to_string()),
        "{inserts:?}"
    );
}

#[test]
fn word_start_matches_beat_longer_prefixes() {
    // `casting-copper` matches `castir` as `CASTI…r`, one consecutive run
    // longer than `casting-iron`'s `CAST`+`IR`, but the latter starts a word.
    const CASTING: &str = r#"{
        "item": {
            "casting-iron": {"type": "item", "name": "casting-iron", "stack_size": 50},
            "casting-copper": {"type": "item", "name": "casting-copper", "stack_size": 50}
        }
    }"#;
    let registry = hactorio_data::postprocess(CASTING, &hactorio_data::Locale::empty()).unwrap();
    let iron = registry.entry("casting-iron").unwrap();

    let cursor = "castir".chars().count();
    let (start, word) = completion_word("castir", cursor);
    let item = completion_items(&registry, &Settings::default(), "castir", start, &word)
        .into_iter()
        .find(|item| item.label == "casting-iron")
        .expect("casting-iron");
    assert_eq!(item.matched, [0, 1, 2, 3, 8, 9], "CAST-IRon");

    let inserts = suggested_ids(&registry, "castir");
    assert_eq!(
        inserts.first(),
        Some(&iron.glyph.to_string()),
        "{inserts:?}"
    );
}

#[test]
fn completion_after_in_only_offers_machines() {
    let registry = fixture_registry();
    let machine = registry
        .entry("assembling-machine-3")
        .unwrap()
        .glyph
        .to_string();
    let plate = registry.entry("iron-plate").unwrap().glyph.to_string();

    let inserts = suggested_ids(&registry, "in asm");
    assert!(inserts.contains(&machine), "{inserts:?}");

    let inserts = suggested_ids(&registry, "in ipl");
    assert!(!inserts.contains(&plate), "{inserts:?}");
}

#[test]
fn completion_fuzzy_matches_highlighted_characters() {
    let registry = fixture_registry();
    let cursor = "eec".chars().count();
    let (start, word) = completion_word("eec", cursor);
    let item = completion_items(&registry, &Settings::default(), "eec", start, &word)
        .into_iter()
        .find(|item| item.label == "electronic-circuit")
        .expect("electronic-circuit");
    let highlighted: String = item
        .matched
        .iter()
        .filter_map(|index| item.label.chars().nth(*index))
        .collect();
    assert_eq!(highlighted, "eec");
}

#[test]
fn aliases_resolve_to_prototypes() {
    const ALIAS_FIXTURE: &str = r#"{
        "item": {
            "electronic-circuit": {"type": "item", "name": "electronic-circuit", "stack_size": 200},
            "advanced-circuit": {"type": "item", "name": "advanced-circuit", "stack_size": 200},
            "processing-unit": {"type": "item", "name": "processing-unit", "stack_size": 100},
            "transport-belt": {"type": "item", "name": "transport-belt", "stack_size": 100},
            "turbo-transport-belt": {"type": "item", "name": "turbo-transport-belt", "stack_size": 100},
            "electromagnetic-plant": {"type": "item", "name": "electromagnetic-plant", "stack_size": 20},
            "automation-science-pack": {"type": "item", "name": "automation-science-pack", "stack_size": 200},
            "military-science-pack": {"type": "item", "name": "military-science-pack", "stack_size": 200},
            "metallurgic-science-pack": {"type": "item", "name": "metallurgic-science-pack", "stack_size": 200},
            "cryogenic-science-pack": {"type": "item", "name": "cryogenic-science-pack", "stack_size": 200}
        }
    }"#;
    let registry =
        hactorio_data::postprocess(ALIAS_FIXTURE, &hactorio_data::Locale::empty()).unwrap();

    let cases = [
        ("green-chip", "green-chip"),
        ("green-circuit", "green-circuit"),
        ("red-chip", "red-chip"),
        ("blue-chip", "blue-chip"),
        ("yellow-belt", "yellow-belt"),
        ("green-belt", "green-belt"),
        ("em-plant", "em-plant"),
        ("red-science-pack", "red-science-pack"),
        ("gray-science-pack", "gray-science-pack"),
        ("black-science-pack", "black-science-pack"),
        ("vulcanus", "vulcanus-science-pack"),
        ("aquilo", "aquilo-science-pack"),
    ];
    for (query, expected) in cases {
        let cursor = query.chars().count();
        let (start, word) = completion_word(query, cursor);
        let items = completion_items(&registry, &Settings::default(), query, start, &word);
        let item = items
            .iter()
            .find(|item| item.label == expected)
            .unwrap_or_else(|| {
                panic!(
                    "{query}: {:?}",
                    items.iter().map(|i| &i.label).collect::<Vec<_>>()
                )
            });
        let matched: String = item
            .matched
            .iter()
            .filter_map(|index| item.label.chars().nth(*index))
            .collect();
        assert_eq!(matched, query, "{query}");
    }

    // The alias, not the canonical name, is displayed when it is the best match.
    let cursor = "green-chip".chars().count();
    let (start, word) = completion_word("green-chip", cursor);
    let items = completion_items(&registry, &Settings::default(), "green-chip", start, &word);
    assert!(items.iter().all(|item| item.label != "electronic-circuit"));
}

#[test]
fn completion_shows_the_prototype_name_after_the_matched_alias() {
    let dir = std::env::temp_dir().join(format!("hactorio-locale-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("recipe-locale.json"),
        r#"{"names": {"electronic-circuit": "Electronic circuit"}}"#,
    )
    .unwrap();
    let registry = hactorio_data::postprocess(FIXTURE, &hactorio_data::Locale::load(&dir)).unwrap();

    let (start, word) = completion_word("green-chip", "green-chip".chars().count());
    let items = completion_items(&registry, &Settings::default(), "green-chip", start, &word);
    let item = items
        .iter()
        .find(|item| item.label == "green-chip")
        .expect("alias row");
    assert_eq!(item.name.as_deref(), Some("Electronic circuit"));
    assert_eq!(item.matched, (0..10).collect::<Vec<_>>());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_display_name_that_adds_words_is_still_matched() {
    let dir = std::env::temp_dir().join(format!("hactorio-display-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("item-locale.json"),
        r#"{"names": {
            "copper-cable": "Personal wire",
            "iron-plate": "Iron plate"
        }}"#,
    )
    .unwrap();
    let registry = hactorio_data::postprocess(FIXTURE, &hactorio_data::Locale::load(&dir)).unwrap();

    let (start, word) = completion_word("personal", "personal".chars().count());
    let items = completion_items(&registry, &Settings::default(), "personal", start, &word);
    let item = items
        .iter()
        .find(|item| item.label == "copper-cable")
        .unwrap_or_else(|| {
            panic!(
                "{:?}",
                items.iter().map(|item| &item.label).collect::<Vec<_>>()
            )
        });
    assert_eq!(item.name.as_deref(), Some("Personal wire"));
    assert_eq!(item.name_matched, (0..8).collect::<Vec<_>>());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_display_name_that_only_reformats_the_id_is_not_matched_again() {
    let dir = std::env::temp_dir().join(format!("hactorio-reformat-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("item-locale.json"),
        r#"{"names": {"iron-plate": "Iron plate"}}"#,
    )
    .unwrap();
    let registry = hactorio_data::postprocess(FIXTURE, &hactorio_data::Locale::load(&dir)).unwrap();

    // Matching `Iron plate` case-sensitively would score above the ID, but the
    // name is only the ID reformatted, so the ID is highlighted instead.
    let (start, word) = completion_word("Iron", "Iron".chars().count());
    let items = completion_items(&registry, &Settings::default(), "Iron", start, &word);
    let item = items
        .iter()
        .find(|item| item.label == "iron-plate")
        .expect("iron-plate row");
    assert_eq!(item.name.as_deref(), Some("Iron plate"));
    assert_eq!(item.matched, (0..4).collect::<Vec<_>>());
    assert!(item.name_matched.is_empty());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_empty_query_lists_everything_the_context_allows() {
    let registry = fixture_registry();
    let iron = registry.entry("iron-plate").unwrap().glyph.to_string();
    let circuit = registry
        .entry("electronic-circuit")
        .unwrap()
        .glyph
        .to_string();
    let water = registry.entry("water").unwrap().glyph.to_string();

    let inserts = suggested_ids(&registry, "");
    assert!(inserts.contains(&iron), "{inserts:?}");
    assert!(inserts.contains(&water), "{inserts:?}");
    assert!(inserts.contains(&"making".to_string()), "{inserts:?}");

    let inserts = suggested_ids(&registry, "making ");
    assert!(inserts.contains(&circuit), "{inserts:?}");
    assert!(!inserts.contains(&water), "{inserts:?}");

    let inserts = suggested_ids(&registry, "speed = 3\n");
    assert!(inserts.contains(&"speed".to_string()), "{inserts:?}");
}

#[test]
fn hidden_items_and_recipes_are_not_suggested() {
    const HIDDEN_FIXTURE: &str = r#"{
        "item": {
            "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100},
            "loader": {"type": "item", "name": "loader", "stack_size": 50, "hidden": true},
            "pistol": {"type": "item", "name": "pistol", "stack_size": 5},
            "rocket-part": {"type": "item", "name": "rocket-part", "stack_size": 5, "hidden": true}
        },
        "recipe": {
            "iron-plate": {"type": "recipe", "name": "iron-plate", "results": [{"type": "item", "name": "iron-plate", "amount": 1}]},
            "loader": {"type": "recipe", "name": "loader", "hidden": true, "ingredients": [{"type": "item", "name": "iron-plate", "amount": 1}], "results": [{"type": "item", "name": "loader", "amount": 1}]},
            "iron-plate-recycling": {"type": "recipe", "name": "iron-plate-recycling", "hidden": true, "ingredients": [{"type": "item", "name": "iron-plate", "amount": 1}], "results": [{"type": "item", "name": "iron-plate", "amount": 1}]},
            "pistol": {"type": "recipe", "name": "pistol", "hidden": true, "ingredients": [{"type": "item", "name": "iron-plate", "amount": 1}], "results": [{"type": "item", "name": "pistol", "amount": 1}]},
            "rocket-part": {"type": "recipe", "name": "rocket-part", "ingredients": [{"type": "item", "name": "iron-plate", "amount": 1}], "results": [{"type": "item", "name": "rocket-part", "amount": 1}]}
        }
    }"#;
    let registry =
        hactorio_data::postprocess(HIDDEN_FIXTURE, &hactorio_data::Locale::empty()).unwrap();
    let glyph = |id: &str| registry.entry(id).unwrap().glyph.to_string();

    // Hidden prototypes stay in the registry, so explicit tokens and recipe
    // inference still resolve them.
    for id in ["loader", "iron-plate-recycling", "pistol", "rocket-part"] {
        assert!(registry.entry(id).is_some(), "{id}");
    }

    let suggestions = suggested_ids(&registry, "");
    assert!(
        suggestions.contains(&glyph("iron-plate")),
        "{suggestions:?}"
    );
    // A visible item keeps its entry even when its recipe is hidden.
    assert!(suggestions.contains(&glyph("pistol")), "{suggestions:?}");
    for hidden in ["loader", "iron-plate-recycling", "rocket-part"] {
        assert!(
            !suggestions.contains(&glyph(hidden)),
            "{hidden}: {suggestions:?}"
        );
    }

    let suggestions = suggested_ids(&registry, "making ");
    assert!(
        suggestions.contains(&glyph("iron-plate")),
        "{suggestions:?}"
    );
    // A visible recipe is still offered when its item is hidden.
    assert!(
        suggestions.contains(&glyph("rocket-part")),
        "{suggestions:?}"
    );
    for hidden in ["loader", "iron-plate-recycling", "pistol"] {
        assert!(
            !suggestions.contains(&glyph(hidden)),
            "{hidden}: {suggestions:?}"
        );
    }
}

#[test]
fn an_empty_query_is_not_cut_to_the_typed_query_limit() {
    let items: Vec<String> = (0..60)
        .map(|index| {
            format!(
                r#""test-item-{index:02}": {{"type": "item", "name": "test-item-{index:02}", "stack_size": 1}}"#
            )
        })
        .collect();
    let fixture = format!(r#"{{"item": {{{}}}}}"#, items.join(","));
    let registry = hactorio_data::postprocess(&fixture, &hactorio_data::Locale::empty()).unwrap();

    let (start, word) = completion_word("", 0);
    let labels: Vec<String> = completion_items(&registry, &Settings::default(), "", start, &word)
        .into_iter()
        .map(|item| item.label)
        .collect();
    assert!(labels.len() > 40, "{}", labels.len());
    assert!(labels.contains(&"test-item-59".to_string()), "{labels:?}");
}

#[test]
fn keywords_are_suggested_in_normal_context() {
    let registry = fixture_registry();
    let inserts = suggested_ids(&registry, "ma");
    assert!(inserts.contains(&"making".to_string()), "{inserts:?}");
    let inserts = suggested_ids(&registry, " i");
    assert!(inserts.contains(&"in".to_string()), "{inserts:?}");
    for unit in ["kJ", "MJ", "GJ", "TJ", "kW", "MW", "GW", "TW"] {
        let inserts = suggested_ids(&registry, unit);
        assert!(inserts.contains(&unit.to_string()), "{unit}: {inserts:?}");
    }
    let inserts = suggested_ids(&registry, "5k");
    assert!(inserts.contains(&"kJ".to_string()), "{inserts:?}");
    assert!(inserts.contains(&"kW".to_string()), "{inserts:?}");
}

#[test]
fn dot_context_offers_properties_and_items() {
    let registry = fixture_registry();
    let text = "x.";
    let cursor = text.chars().count();
    let (start, word) = completion_word(text, cursor);
    assert_eq!(start, cursor);
    assert!(word.is_empty());
    let items = completion_items(&registry, &Settings::default(), text, start, &word);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&".rate"), "{labels:?}");
    assert!(labels.contains(&".capacity"), "{labels:?}");
    // `in`, `out`, and `net` are process selectors, so they are properties
    // after a dot even though `in` is also an operator.
    assert!(labels.contains(&".in"), "{labels:?}");
    assert!(labels.contains(&".out"), "{labels:?}");
    assert!(labels.contains(&".net"), "{labels:?}");
    assert!(labels.contains(&"iron-plate"), "{labels:?}");

    let text = "x.ra";
    let cursor = text.chars().count();
    let (start, word) = completion_word(text, cursor);
    assert_eq!(word, "ra");
    let items = completion_items(&registry, &Settings::default(), text, start, &word);
    assert_eq!(items.first().map(|item| item.insert.as_str()), Some("rate"));
}

#[test]
fn completion_starts_after_pua_or_digit() {
    let registry = fixture_registry();
    let glyph = registry.entry("iron-plate").unwrap().glyph;
    let text = format!("{glyph}ra");
    let (start, word) = completion_word(&text, text.chars().count());
    assert_eq!(start, 1);
    assert_eq!(word, "ra");

    let (start, word) = completion_word("3ra", 3);
    assert_eq!(start, 1);
    assert_eq!(word, "ra");
}

#[test]
fn stack_signals_rank_highest_for_sta() {
    let registry = fixture_registry();
    let text = "sta";
    let cursor = text.chars().count();
    let (start, word) = completion_word(text, cursor);
    let items = completion_items(&registry, &Settings::default(), text, start, &word);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels.first().copied(), Some("stack-size"), "{labels:?}");
    assert_eq!(labels.get(1).copied(), Some("stacked-belt"), "{labels:?}");
}

#[test]
fn exact_matches_rank_first() {
    let registry = fixture_registry();
    let inserts = suggested_ids(&registry, "s");
    assert_eq!(
        inserts.first().map(String::as_str),
        Some("s"),
        "{inserts:?}"
    );

    let inserts = suggested_ids(&registry, "w = 3\nw");
    assert_eq!(
        inserts.first().map(String::as_str),
        Some("w"),
        "{inserts:?}"
    );
    assert_eq!(inserts.get(1).map(String::as_str), Some("W"), "{inserts:?}");
}

#[test]
fn property_completions_use_dotted_names_and_glyphs() {
    let registry = fixture_registry();
    let text = "x.";
    let cursor = text.chars().count();
    let (start, word) = completion_word(text, cursor);
    let items = completion_items(&registry, &Settings::default(), text, start, &word);
    let rate = items.iter().find(|item| item.insert == "rate").unwrap();
    assert_eq!(rate.label, ".rate");
    assert_eq!(rate.glyphs, hactorio_data::EMPTY_GLYPH.to_string());
    let time = items
        .iter()
        .find(|item| item.insert != "time" && item.label == ".time");
    assert!(time.is_some(), "expected a .time glyph entry");
    assert!(!items.iter().any(|item| item.insert == "time"));
    assert!(!items.iter().any(|item| item.insert == "electric"));
    assert!(!items.iter().any(|item| item.insert == "slots"));
    assert!(items.iter().any(|item| item.label == ".module-slots"));
}

#[test]
fn variables_are_suggested_after_their_definition() {
    let registry = fixture_registry();
    let inserts = suggested_ids(&registry, "rate = 3 :iron-plate: / second\nra");
    assert!(inserts.contains(&"rate".to_string()), "{inserts:?}");

    let inserts = suggested_ids(&registry, "ra\nrate = 3 :iron-plate: / second");
    assert!(!inserts.contains(&"rate".to_string()), "{inserts:?}");
}

#[test]
fn variables_are_suggested_after_a_compound_definition() {
    let registry = fixture_registry();
    let inserts = suggested_ids(
        &registry,
        "rate = 3 :iron-plate: / second\nrate += 1 :iron-plate: / second\nra",
    );
    assert!(inserts.contains(&"rate".to_string()), "{inserts:?}");
}

#[test]
fn variables_outrank_similar_prototype_matches() {
    let registry = fixture_registry();
    let inserts = suggested_ids(&registry, "cir = 3\nci");
    assert_eq!(inserts.first().map(String::as_str), Some("cir"));
}

#[test]
fn variables_are_not_suggested_in_machine_or_recipe_contexts() {
    let registry = fixture_registry();
    let machine = suggested_ids(&registry, "rate = 3\nin ra");
    assert!(!machine.contains(&"rate".to_string()), "{machine:?}");

    let recipe = suggested_ids(&registry, "rate = 3\n:assembling-machine-3: making ra");
    assert!(!recipe.contains(&"rate".to_string()), "{recipe:?}");
}

#[test]
fn variable_rows_use_the_variable_signal() {
    let registry = fixture_registry();
    let text = "rate = 3 :iron-plate: / second\nra";
    let cursor = text.chars().count();
    let (start, word) = completion_word(text, cursor);
    let items = completion_items(&registry, &Settings::default(), text, start, &word);
    let item = items
        .iter()
        .find(|item| item.insert == "rate")
        .expect("variable row");

    assert_eq!(item.glyphs, crate::glyphs::VARIABLE.to_string());
    let font_id = egui::FontId::monospace(13.0);
    let job = completion_layout_job(item, 13, false, &font_id, &egui::Visuals::dark());
    let glyph_section = &job.sections[0];
    assert_eq!(
        job.text[glyph_section.byte_range.start.0..glyph_section.byte_range.end.0].trim_end(),
        crate::glyphs::VARIABLE.to_string()
    );
    assert_eq!(glyph_section.format.extra_letter_spacing, 0.0);
}

#[test]
fn typing_a_dot_opens_property_completions() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("dot-trigger", document);
    harness.run();
    type_into_editor(&mut harness, " x.");
    let app = harness.state().as_ref().expect("app");
    assert!(
        !app.completion.items.is_empty(),
        "completions should open after a dot"
    );
    assert!(
        app.completion
            .items
            .iter()
            .any(|item| item.insert == "rate"),
        "{:?}",
        app.completion
            .items
            .iter()
            .map(|item| &item.insert)
            .collect::<Vec<_>>()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_space_after_the_dot_still_opens_property_completions() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("dot-space", document);
    harness.run();
    type_into_editor(&mut harness, " x. ");
    let app = harness.state().as_ref().expect("app");
    assert!(
        app.completion
            .items
            .iter()
            .any(|item| item.insert == "rate"),
        "{:?}",
        app.completion
            .items
            .iter()
            .map(|item| &item.insert)
            .collect::<Vec<_>>()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn accepting_an_alias_replaces_the_whole_alias() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("alias-accept", document);
    harness.run();
    type_into_editor(&mut harness, " green-chip");
    harness.key_press(Key::Enter);
    harness.run();

    let app = harness.state().as_ref().expect("app");
    let glyph = app
        .registry
        .as_ref()
        .expect("registry")
        .entry("electronic-circuit")
        .expect("electronic-circuit")
        .glyph;
    assert_eq!(app.tabs[0].text, format!("# Main {glyph}\n"));

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn completion_after_making_only_offers_recipes() {
    let registry = fixture_registry();
    let circuit = registry
        .entry("electronic-circuit")
        .unwrap()
        .glyph
        .to_string();
    let water = registry.entry("water").unwrap().glyph.to_string();

    let inserts = suggested_ids(&registry, "making :circ");
    assert!(inserts.contains(&circuit), "{inserts:?}");

    let inserts = suggested_ids(&registry, "making :water");
    assert!(!inserts.contains(&water), "{inserts:?}");
}

#[test]
fn focusing_a_word_does_not_open_completion() {
    let document = editor_document("# Main\n:iron\n", 1, 5);
    let (mut harness, dir) = completion_harness("focus", document);
    harness.run();

    let app = harness.state().as_ref().expect("app");
    assert!(app.completion.items.is_empty());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn typing_letters_opens_completion() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("letters", document);
    harness.run();

    type_into_editor(&mut harness, " ");
    type_into_editor(&mut harness, "s");
    type_into_editor(&mut harness, "m");
    type_into_editor(&mut harness, "3");

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.completion.word, "sm3");
    assert!(!app.completion.items.is_empty());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn typing_digits_does_not_open_completion() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("digits", document);
    harness.run();

    type_into_editor(&mut harness, " ");
    type_into_editor(&mut harness, "3");
    assert!(
        harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .is_empty()
    );

    type_into_editor(&mut harness, "4");
    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.completion.word, "34");
    assert!(app.completion.items.is_empty());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn horizontal_arrow_closes_completion_and_moves_the_caret() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("left", document);
    harness.run();
    type_into_editor(&mut harness, " sm3");
    assert!(
        !harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .is_empty()
    );

    harness.key_press(Key::ArrowLeft);
    harness.run();
    assert!(
        harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .is_empty()
    );

    type_into_editor(&mut harness, "X");
    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Main smX3\n");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn vertical_arrow_moves_completion_selection() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("down", document);
    harness.run();
    type_into_editor(&mut harness, " i");
    {
        let app = harness.state().as_ref().expect("app");
        assert!(app.completion.items.len() > 1);
        assert_eq!(app.completion.selected, 0);
    }

    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(
        harness.state().as_ref().expect("app").completion.selected,
        1
    );

    type_into_editor(&mut harness, "Z");
    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Main iZ\n");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn tab_moves_focus_out_of_the_editor() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("tab", document);
    harness.run();
    type_into_editor(&mut harness, "x");

    harness.key_press(Key::Tab);
    harness.run();
    type_into_editor(&mut harness, "Y");
    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Mainx\n");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn shift_tab_moves_focus_out_of_the_editor() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("shift-tab", document);
    harness.run();
    type_into_editor(&mut harness, "x");

    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run();
    type_into_editor(&mut harness, "Y");
    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Mainx\n");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn tab_accepts_completion_and_keeps_editor_focus() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("tab-accept", document);
    harness.run();
    type_into_editor(&mut harness, " :iron");
    let glyph = harness
        .state()
        .as_ref()
        .expect("app")
        .registry
        .as_ref()
        .expect("registry")
        .entry("iron-plate")
        .expect("iron-plate")
        .glyph;

    harness.key_press(Key::Tab);
    harness.run();
    assert!(
        harness.state().as_ref().expect("app").tabs[0]
            .text
            .contains(glyph)
    );

    type_into_editor(&mut harness, "Z");
    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, format!("# Main {glyph}Z\n"));

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn shift_tab_moves_focus_while_completion_is_open() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("shift-tab-popup", document);
    harness.run();
    type_into_editor(&mut harness, " i");
    assert!(
        !harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .is_empty()
    );

    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run();
    type_into_editor(&mut harness, "Y");
    let app = harness.state().as_ref().expect("app");
    assert!(app.completion.items.is_empty());
    assert_eq!(app.tabs[0].text, "# Main i\n");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn completion_popup_fits_the_widest_entry() {
    const WIDE: &str = r#"{
        "item": {
            "short": {"type": "item", "name": "short", "stack_size": 1},
            "long-prototype-name-that-needs-a-wide-popup": {
                "type": "item",
                "name": "long-prototype-name-that-needs-a-wide-popup",
                "stack_size": 1
            }
        }
    }"#;
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness_with("wide", document, WIDE);
    harness.run();
    let id = egui::Id::unique(("completion", 0usize));

    type_into_editor(&mut harness, " sho");
    let short_rect = harness
        .ctx
        .memory(|memory| memory.area_rect(id))
        .expect("popup");

    type_into_editor(&mut harness, " lon");
    let long_rect = harness
        .ctx
        .memory(|memory| memory.area_rect(id))
        .expect("popup");

    assert!(long_rect.width() > short_rect.width(), "{short_rect:?}");
    assert!(long_rect.width() > 300.0, "{long_rect:?}");
    // A wrapped row would make the one-entry popup taller.
    assert!(
        long_rect.height() <= short_rect.height() + 2.0,
        "{short_rect:?} -> {long_rect:?}"
    );

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn completion_popup_grows_with_more_matches() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("growth", document);
    harness.run();

    // `w` matches ten rows, which fits under the popup's cap; `e` matches many
    // more, so the popup grows until it caps.
    type_into_editor(&mut harness, " w");
    let id = egui::Id::unique(("completion", 0usize));
    let short_items = harness
        .state()
        .as_ref()
        .expect("app")
        .completion
        .items
        .len();
    let short = harness
        .ctx
        .memory(|memory| memory.area_rect(id))
        .expect("popup")
        .height();

    type_into_editor(&mut harness, " ");
    type_into_editor(&mut harness, "e");
    let long_items = harness
        .state()
        .as_ref()
        .expect("app")
        .completion
        .items
        .len();
    let tall = harness
        .ctx
        .memory(|memory| memory.area_rect(id))
        .expect("popup")
        .height();

    assert!(long_items > short_items, "{short_items} -> {long_items}");
    assert!(tall > short, "{short} -> {tall}");
    assert!(tall <= COMPLETION_MAX_HEIGHT + 10.0, "{tall}");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn escape_closes_completion_and_keeps_editor_focus() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) = completion_harness("escape", document);
    harness.run();
    type_into_editor(&mut harness, " i");
    assert!(
        !harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .is_empty()
    );

    harness.key_press(Key::Escape);
    harness.run();
    assert!(
        harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .is_empty()
    );

    type_into_editor(&mut harness, "Z");
    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Main iZ\n");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn completion_accepts_with_enter_and_inserts_a_glyph() {
    if std::env::var_os("HACTORIO_UI_SNAPSHOT").is_none() {
        return;
    }
    let app_dir = data_dir();
    if !app_dir.join("cache.json").is_file() || locate().is_none() {
        eprintln!("skipping: no game data cache or Factorio install");
        return;
    }

    let mut document = sample_document();
    let line = document.cursors[0].line;
    document.cursors[0].column = document.tabs[0]
        .split('\n')
        .nth(line)
        .map(|text| text.chars().count())
        .unwrap_or(0);

    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_accessibility_check(false)
        .allow_missing_glyphs()
        .wgpu()
        .with_max_steps(60)
        .build_ui_state(
            move |ui, app: &mut Option<HactorioApp>| {
                let app = app.get_or_insert_with(|| {
                    HactorioApp::new(ui.ctx(), document.clone(), app_dir.clone())
                });
                app.ui(ui);
            },
            None,
        );

    harness.run();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(" :iron".to_string()));
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();

    let app = harness.state().as_ref().expect("app");
    let glyph = app
        .registry
        .as_ref()
        .unwrap()
        .entry("iron-ore")
        .unwrap()
        .glyph;
    assert!(app.tabs[0].text.contains(glyph), "{:?}", app.tabs[0].text);
    assert_eq!(
        app.tabs[0].text.split('\n').count(),
        8,
        "Enter must accept, not insert a newline"
    );
}

#[test]
fn module_abbreviations_rank_their_tier_first() {
    let modules: Vec<String> = [
        ("speed-module", "speed", 1),
        ("speed-module-2", "speed", 2),
        ("speed-module-3", "speed", 3),
        ("productivity-module", "productivity", 1),
        ("productivity-module-2", "productivity", 2),
        ("productivity-module-3", "productivity", 3),
        ("efficiency-module", "efficiency", 1),
        ("efficiency-module-2", "efficiency", 2),
        ("efficiency-module-3", "efficiency", 3),
        ("quality-module", "quality", 1),
        ("quality-module-2", "quality", 2),
        ("quality-module-3", "quality", 3),
    ]
    .into_iter()
    .map(|(id, category, tier)| {
        format!(
            r#""{id}": {{"type": "module", "name": "{id}", "category": "{category}", "tier": {tier}, "stack_size": 50, "effect": {{}}}}"#
        )
    })
    .collect();
    let fixture = format!(r#"{{"module": {{{}}}}}"#, modules.join(","));
    let registry = hactorio_data::postprocess(&fixture, &hactorio_data::Locale::empty()).unwrap();

    for (alias, id) in [
        ("speed-mod-1", "speed-module"),
        ("speed-mod-2", "speed-module-2"),
        ("speed-mod-3", "speed-module-3"),
        ("prod-mod-1", "productivity-module"),
        ("prod-mod-2", "productivity-module-2"),
        ("prod-mod-3", "productivity-module-3"),
        ("eff-mod-1", "efficiency-module"),
        ("eff-mod-2", "efficiency-module-2"),
        ("eff-mod-3", "efficiency-module-3"),
        ("qual-mod-1", "quality-module"),
        ("qual-mod-2", "quality-module-2"),
        ("qual-mod-3", "quality-module-3"),
    ] {
        let expected = registry.entry(id).unwrap().glyph.to_string();
        for query in [alias.to_string(), format!(":{alias}")] {
            let inserts = suggested_ids(&registry, &query);
            assert_eq!(inserts.first(), Some(&expected), "{query}: {inserts:?}");
        }
    }
}

#[test]
fn exact_names_outrank_near_miss_fuzzy_matches() {
    fn entry(id: &str, display_name: Option<&str>) -> hactorio_data::Entry {
        hactorio_data::Entry {
            id: id.into(),
            glyph: 'a',
            display_name: display_name.map(Into::into),
            icon: None,
            item: None,
            fluid: false,
            module: None,
            recipe: None,
            beacon: None,
            equipment: None,
            hidden: false,
        }
    }

    let fuzzy = entry("power-armor-mk2", Some("Power armor MK2"));
    let exact = entry("pm2", None);
    let fuzzy_score = best_name_match(&fuzzy, "pm2").unwrap().0;
    let exact_score = best_name_match(&exact, "pm2").unwrap().0;
    // `Power armor MK2` only reformats its ID, so it is not searched again.
    assert!(exact_score > fuzzy_score, "{exact_score} <= {fuzzy_score}");
}

const AUTOCOMPLETE_FIXTURE: &str = r#"{
  "item": {
    "transport-belt": {"type": "item", "name": "transport-belt", "stack_size": 100},
    "fast-transport-belt": {"type": "item", "name": "fast-transport-belt", "stack_size": 100},
    "express-transport-belt": {"type": "item", "name": "express-transport-belt", "stack_size": 100},
    "turbo-transport-belt": {"type": "item", "name": "turbo-transport-belt", "stack_size": 100},
    "underground-belt": {"type": "item", "name": "underground-belt", "stack_size": 50},
    "fast-underground-belt": {"type": "item", "name": "fast-underground-belt", "stack_size": 50},
    "express-underground-belt": {"type": "item", "name": "express-underground-belt", "stack_size": 50},
    "turbo-underground-belt": {"type": "item", "name": "turbo-underground-belt", "stack_size": 50},
    "splitter": {"type": "item", "name": "splitter", "stack_size": 50},
    "fast-splitter": {"type": "item", "name": "fast-splitter", "stack_size": 50},
    "express-splitter": {"type": "item", "name": "express-splitter", "stack_size": 50},
    "turbo-splitter": {"type": "item", "name": "turbo-splitter", "stack_size": 50},
    "petroleum-gas-barrel": {"type": "item", "name": "petroleum-gas-barrel", "stack_size": 10},
    "atomic-bomb": {"type": "item", "name": "atomic-bomb", "stack_size": 10}
  },
  "ammo": {
    "firearm-magazine": {"type": "ammo", "name": "firearm-magazine", "stack_size": 100},
    "piercing-rounds-magazine": {"type": "ammo", "name": "piercing-rounds-magazine", "stack_size": 100},
    "uranium-rounds-magazine": {"type": "ammo", "name": "uranium-rounds-magazine", "stack_size": 100}
  },
  "fluid": {
    "lubricant": {"type": "fluid", "name": "lubricant"}
  },
  "module": {
    "speed-module": {"type": "module", "name": "speed-module", "category": "speed", "tier": 1, "stack_size": 50, "effect": {"speed": 0.2}},
    "speed-module-2": {"type": "module", "name": "speed-module-2", "category": "speed", "tier": 2, "stack_size": 50, "effect": {"speed": 0.3}},
    "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}},
    "productivity-module": {"type": "module", "name": "productivity-module", "category": "productivity", "tier": 1, "stack_size": 50, "effect": {"productivity": 0.04}},
    "productivity-module-2": {"type": "module", "name": "productivity-module-2", "category": "productivity", "tier": 2, "stack_size": 50, "effect": {"productivity": 0.06}},
    "productivity-module-3": {"type": "module", "name": "productivity-module-3", "category": "productivity", "tier": 3, "stack_size": 50, "effect": {"productivity": 0.1}},
    "efficiency-module": {"type": "module", "name": "efficiency-module", "category": "efficiency", "tier": 1, "stack_size": 50, "effect": {"consumption": -0.3}},
    "efficiency-module-2": {"type": "module", "name": "efficiency-module-2", "category": "efficiency", "tier": 2, "stack_size": 50, "effect": {"consumption": -0.4}},
    "efficiency-module-3": {"type": "module", "name": "efficiency-module-3", "category": "efficiency", "tier": 3, "stack_size": 50, "effect": {"consumption": -0.5}},
    "quality-module": {"type": "module", "name": "quality-module", "category": "quality", "tier": 1, "stack_size": 50, "effect": {"quality": 0.01}},
    "quality-module-2": {"type": "module", "name": "quality-module-2", "category": "quality", "tier": 2, "stack_size": 50, "effect": {"quality": 0.02}},
    "quality-module-3": {"type": "module", "name": "quality-module-3", "category": "quality", "tier": 3, "stack_size": 50, "effect": {"quality": 0.025}}
  }
}"#;

fn autocomplete_registry() -> Registry {
    hactorio_data::postprocess(AUTOCOMPLETE_FIXTURE, &hactorio_data::Locale::empty()).unwrap()
}

fn completion_labels(registry: &Registry, text: &str) -> Vec<String> {
    let cursor = text.chars().count();
    let (start, word) = completion_word(text, cursor);
    completion_items(registry, &Settings::default(), text, start, &word)
        .into_iter()
        .map(|item| item.label)
        .collect()
}

fn first_completion(registry: &Registry, text: &str) -> CompletionItem {
    let cursor = text.chars().count();
    let (start, word) = completion_word(text, cursor);
    let items = completion_items(registry, &Settings::default(), text, start, &word);
    assert!(
        !items.is_empty(),
        "{text}: no completions, labels {:?}",
        items.iter().map(|item| &item.label).collect::<Vec<_>>()
    );
    items[0].clone()
}

#[test]
fn belt_color_tokens_pick_their_belt() {
    let registry = autocomplete_registry();
    for (query, expected, canonical) in [
        ("yb", "yellow-belt", "transport-belt"),
        ("rb", "red-belt", "fast-transport-belt"),
        ("bb", "blue-belt", "express-transport-belt"),
        ("gb", "green-belt", "turbo-transport-belt"),
    ] {
        let labels = completion_labels(&registry, query);
        assert_eq!(
            labels.first().map(String::as_str),
            Some(expected),
            "{query}: {labels:?}"
        );
        assert_eq!(
            first_completion(&registry, query).insert,
            registry.entry(canonical).unwrap().glyph.to_string(),
            "{query}"
        );
    }
}

#[test]
fn a_quantity_offers_items_after_a_dot() {
    let registry = fixture_registry();
    let text = "q = 2 :iron-plate:\nq.";
    let labels: Vec<String> = {
        let cursor = text.chars().count();
        let (start, word) = completion_word(text, cursor);
        completion_items(&registry, &Settings::default(), text, start, &word)
            .into_iter()
            .map(|item| item.label)
            .collect()
    };
    assert!(
        labels.iter().any(|label| label == "iron-plate"),
        "{labels:?}"
    );
    assert!(labels.iter().any(|label| label == "water"), "{labels:?}");
    assert!(labels.iter().any(|label| label == ".item"), "{labels:?}");
    // A selector can be qualified, so the qualities are offered here too.
    for quality in ["normal", "rare", "uncommon"] {
        assert!(labels.iter().any(|label| label == quality), "{labels:?}");
    }
}

#[test]
fn making_offers_qualities_alongside_recipes() {
    let registry = fixture_registry();
    let text = "making :";
    let labels: Vec<String> = {
        let cursor = text.chars().count();
        let (start, word) = completion_word(text, cursor);
        completion_items(&registry, &Settings::default(), text, start, &word)
            .into_iter()
            .map(|item| item.label)
            .collect()
    };
    assert!(
        labels.iter().any(|label| label == "electronic-circuit"),
        "{labels:?}"
    );
    for quality in ["normal", "rare", "legendary"] {
        assert!(labels.iter().any(|label| label == quality), "{labels:?}");
    }
    // Hidden recipes are reachable by name but never suggested.
    assert!(
        !labels.iter().any(|label| label == "iron-plate-recycling"),
        "{labels:?}"
    );
}

#[test]
fn a_process_offers_its_signals_after_a_dot() {
    let registry = autocomplete_registry();
    let text = "q = 1 :assembling-machine-3: making (:normal: :iron-gear-wheel:)\nq.";
    let labels: Vec<String> = {
        let cursor = text.chars().count();
        let (start, word) = completion_word(text, cursor);
        completion_items(&registry, &Settings::default(), text, start, &word)
            .into_iter()
            .map(|item| item.label)
            .collect()
    };
    for expected in [
        ".speed",
        ".electric",
        ".in",
        ".out",
        ".net",
        ".item",
        ".fluid",
        ".machines",
    ] {
        assert!(
            labels.iter().any(|label| label == expected),
            "{expected} missing from {labels:?}"
        );
    }
}

#[test]
fn quality_spellings_share_one_completion_row() {
    let registry = autocomplete_registry();
    let items = completion_items(&registry, &Settings::default(), "", 0, "");
    for quality in hactorio_core::new_eval::Quality::ALL {
        let glyph = quality.glyph().to_string();
        let rows: Vec<&str> = items
            .iter()
            .filter(|item| item.insert == glyph)
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(rows.len(), 1, "{}: {rows:?}", quality.name());
        assert_eq!(rows[0], quality.name(), "{}", quality.name());
    }
}

#[test]
fn qualities_are_offered_with_their_own_glyph() {
    let registry = autocomplete_registry();
    for (query, id) in [("rar", "rare"), ("leg", "legendary"), ("nor", "normal")] {
        let item = first_completion(&registry, query);
        assert_eq!(item.label, id, "{query}");
        let glyph = hactorio_data::quality_by_id(id).unwrap().glyph;
        assert_eq!(item.insert, glyph.to_string(), "{query}");
        assert_eq!(item.glyphs, glyph.to_string(), "{query}");
    }
}

#[test]
fn quality_shortcuts_rank_their_quality_first() {
    let registry = autocomplete_registry();
    for (query, id) in [
        ("q1", "quality-1"),
        ("q2", "quality-2"),
        ("q3", "quality-3"),
        ("q4", "quality-4"),
        ("q5", "quality-5"),
    ] {
        let item = first_completion(&registry, query);
        assert_eq!(item.label, id, "{query}: got {:?}", item.label);
        let quality = hactorio_core::new_eval::Quality::from_name(id).unwrap();
        assert_eq!(item.insert, quality.glyph().to_string(), "{query}");
    }

    // The spelled-out aliases resolve too, `common` to the normal quality.
    let item = first_completion(&registry, "common");
    assert_eq!(item.label, "common");
    assert_eq!(
        item.insert,
        hactorio_core::new_eval::Quality::Normal.glyph().to_string()
    );
}

#[test]
fn operators_are_suggested_without_with() {
    let registry = autocomplete_registry();
    for keyword in ["in", "into", "from", "making"] {
        let labels: Vec<String> = {
            let (start, word) = completion_word(keyword, keyword.chars().count());
            completion_items(&registry, &Settings::default(), keyword, start, &word)
                .into_iter()
                .map(|item| item.label)
                .collect()
        };
        assert!(labels.iter().any(|label| label == keyword), "{keyword}");
    }
    let (start, word) = completion_word("wit", 3);
    let labels: Vec<String> =
        completion_items(&registry, &Settings::default(), "wit", start, &word)
            .into_iter()
            .map(|item| item.label)
            .collect();
    assert!(!labels.iter().any(|label| label == "with"), "{labels:?}");
}

#[test]
fn stacked_belt_tokens_insert_the_stacked_prefix() {
    let registry = autocomplete_registry();
    let stacked = hactorio_data::SIGNALS
        .iter()
        .find(|signal| signal.id == "stacked-belt")
        .unwrap()
        .glyph;
    for (query, label, belt_id) in [
        ("syb", "stacked-yellow-belt", "transport-belt"),
        ("srb", "stacked-red-belt", "fast-transport-belt"),
        ("sbb", "stacked-blue-belt", "express-transport-belt"),
        ("sgb", "stacked-green-belt", "turbo-transport-belt"),
    ] {
        let item = first_completion(&registry, query);
        assert_eq!(item.label, label, "{query}");
        let belt = registry.entry(belt_id).unwrap().glyph;
        assert_eq!(item.insert, format!("{stacked}{belt}"), "{query}");
    }
}

#[test]
fn accepting_a_stacked_belt_row_inserts_both_glyphs() {
    let document = editor_document("# Main\n", 0, 6);
    let (mut harness, dir) =
        completion_harness_with("stacked-accept", document, AUTOCOMPLETE_FIXTURE);
    harness.run();
    type_into_editor(&mut harness, " :syb");
    let labels: Vec<String> = harness
        .state()
        .as_ref()
        .expect("app")
        .completion
        .items
        .iter()
        .map(|item| item.label.clone())
        .collect();
    assert_eq!(
        labels.first().map(String::as_str),
        Some("stacked-yellow-belt"),
        "{labels:?}"
    );

    harness.key_press(Key::Enter);
    harness.run();

    let registry = autocomplete_registry();
    let stacked = hactorio_data::SIGNALS
        .iter()
        .find(|signal| signal.id == "stacked-belt")
        .unwrap()
        .glyph;
    let belt = registry.entry("transport-belt").unwrap().glyph;
    let text = &harness.state().as_ref().expect("app").tabs[0].text;
    assert_eq!(*text, format!("# Main {stacked}{belt}\n"));

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn color_aliases_name_underground_belts_and_splitters() {
    let registry = autocomplete_registry();
    let cases = [
        ("yellow-underground-belt", "underground-belt"),
        ("red-underground-belt", "fast-underground-belt"),
        ("blue-underground-belt", "express-underground-belt"),
        ("green-underground-belt", "turbo-underground-belt"),
        ("yellow-splitter", "splitter"),
        ("red-splitter", "fast-splitter"),
        ("blue-splitter", "express-splitter"),
        ("green-splitter", "turbo-splitter"),
    ];
    for (alias, canonical) in cases {
        let item = first_completion(&registry, alias);
        assert_eq!(item.label, alias);
        assert_eq!(
            item.insert,
            registry.entry(canonical).unwrap().glyph.to_string()
        );
    }
}

#[test]
fn ammo_aliases_name_their_magazines() {
    let registry = autocomplete_registry();
    for (alias, canonical) in [
        ("yellow-ammo", "firearm-magazine"),
        ("yellow-rounds-magazine", "firearm-magazine"),
        ("red-ammo", "piercing-rounds-magazine"),
        ("red-rounds-magazine", "piercing-rounds-magazine"),
        ("piercing-ammo", "piercing-rounds-magazine"),
        ("green-ammo", "uranium-rounds-magazine"),
        ("green-rounds-magazine", "uranium-rounds-magazine"),
    ] {
        let item = first_completion(&registry, alias);
        assert_eq!(item.label, alias);
        assert_eq!(
            item.insert,
            registry.entry(canonical).unwrap().glyph.to_string(),
            "{alias}"
        );
    }
}

#[test]
fn community_aliases_cover_nuclear_bomb_and_lubricant() {
    let registry = autocomplete_registry();
    for (alias, canonical) in [("nuclear-bomb", "atomic-bomb"), ("lube", "lubricant")] {
        let item = first_completion(&registry, alias);
        assert_eq!(
            item.insert,
            registry.entry(canonical).unwrap().glyph.to_string()
        );
    }
}

#[test]
fn module_abbreviations_pick_their_module() {
    let registry = autocomplete_registry();
    let cases = [
        ("sm1", "speed-module"),
        ("pm1", "productivity-module"),
        ("em1", "efficiency-module"),
        ("qm1", "quality-module"),
        ("sm2", "speed-module-2"),
        ("pm2", "productivity-module-2"),
        ("em2", "efficiency-module-2"),
        ("qm2", "quality-module-2"),
        ("sm3", "speed-module-3"),
        ("pm3", "productivity-module-3"),
        ("em3", "efficiency-module-3"),
        ("qm3", "quality-module-3"),
    ];
    for (query, canonical) in cases {
        let labels = completion_labels(&registry, query);
        let expected = registry.entry(canonical).unwrap().glyph.to_string();
        assert_eq!(
            first_completion(&registry, query).insert,
            expected,
            "{query}: {labels:?}"
        );
    }
}

#[test]
fn completion_after_dot_offers_every_unit() {
    let registry = fixture_registry();
    let text = ":assembling-machine-3: making :electronic-circuit:.";
    let labels = completion_labels(&registry, text);
    // Absent units read as zero, so they stay available.
    for label in ["iron-plate", "copper-cable", "iron-gear-wheel"] {
        assert!(
            labels.iter().any(|item| item == label),
            "{label}: {labels:?}"
        );
    }
    assert!(!labels.iter().any(|label| label == "electronic-circuit"));
    // Signals that only work as prefixes are not selectors.
    assert!(
        !labels.iter().any(|label| label == "stack-size"),
        "{labels:?}"
    );
    assert!(
        !labels.iter().any(|label| label == "stacked-belt"),
        "{labels:?}"
    );
}

#[test]
fn completion_after_dot_ranks_present_units_above_absent_ones() {
    let registry = fixture_registry();
    let labels = completion_labels(
        &registry,
        ":assembling-machine-3: making :electronic-circuit:.ir",
    );
    let present = labels.iter().position(|label| label == "iron-plate");
    let absent = labels.iter().position(|label| label == "iron-gear-wheel");
    let (Some(present), Some(absent)) = (present, absent) else {
        panic!("expected both iron rows, got {labels:?}");
    };
    assert!(present < absent, "{labels:?}");
}

#[test]
fn completion_after_dot_hides_abs_for_a_process() {
    let registry = fixture_registry();
    let process = completion_labels(&registry, "(:iron-plate: in :assembling-machine-3:).");
    assert!(
        process.iter().any(|label| label == ".crafts"),
        "{process:?}"
    );
    assert!(!process.iter().any(|label| label == ".abs"), "{process:?}");
    let item = completion_labels(&registry, "2 :iron-plate:.");
    assert!(item.iter().any(|label| label == ".abs"), "{item:?}");
}

#[test]
fn completion_after_dot_hides_properties_the_value_lacks() {
    let registry = fixture_registry();
    let labels = completion_labels(&registry, "2 :assembling-machine-3:.");
    assert!(labels.iter().any(|label| label == ".speed"), "{labels:?}");
    assert!(
        labels.iter().any(|label| label == ".machines"),
        "{labels:?}"
    );
    assert!(!labels.iter().any(|label| label == ".rate"), "{labels:?}");
    assert!(!labels.iter().any(|label| label == ".time"), "{labels:?}");
    assert!(!labels.iter().any(|label| label == ".crafts"), "{labels:?}");
}

#[test]
fn completion_after_dot_fuzzy_matches_signal_selectors() {
    let registry = fixture_registry();
    let process = ":assembling-machine-3: making :electronic-circuit:.";
    assert!(
        completion_labels(&registry, &format!("{process}ectric"))
            .iter()
            .any(|label| label == ".electric")
    );
    // A typed prefix keeps signal rows ahead of the property rows.
    let labels = completion_labels(&registry, &format!("{process}cr"));
    assert_eq!(
        labels.first().map(String::as_str),
        Some(".crafts"),
        "{labels:?}"
    );
    assert!(
        labels.iter().any(|label| label == ".electric"),
        "{labels:?}"
    );
}

#[test]
fn completion_after_dot_reads_the_operand_of_a_reassignment() {
    let registry = fixture_registry();
    let process = "p = :assembling-machine-3: making :electronic-circuit:";
    // The dot follows the `p` operand, so its value is the process `p` holds.
    // Evaluating the whole statement would add the process to itself instead.
    let labels = completion_labels(&registry, &format!("{process}\np += p."));
    assert!(labels.iter().any(|label| label == ".crafts"), "{labels:?}");
    assert!(!labels.iter().any(|label| label == ".rate"), "{labels:?}");
    // Qualities are selectors too, so skip them when looking for the unit.
    let qualifiers: Vec<&str> = hactorio_core::new_eval::Quality::ALL
        .iter()
        .flat_map(|quality| quality.aliases().iter().copied())
        .collect();
    let first_unit = labels
        .iter()
        .find(|label| !label.starts_with('.') && !qualifiers.contains(&label.as_str()));
    assert_eq!(
        first_unit.map(String::as_str),
        Some("iron-plate"),
        "{labels:?}"
    );

    let labels = completion_labels(&registry, &format!("{process}\np += p.:ir"));
    let present = labels.iter().position(|label| label == "iron-plate");
    let absent = labels.iter().position(|label| label == "iron-gear-wheel");
    let (Some(present), Some(absent)) = (present, absent) else {
        panic!("expected both iron rows, got {labels:?}");
    };
    assert!(present < absent, "{labels:?}");
}
