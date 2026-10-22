use super::*;

fn suggested_ids(registry: &Registry, text: &str) -> Vec<String> {
    let cursor = text.chars().count();
    let (start, word) = completion_word(text, cursor);
    completion_items(registry, text, start, &word)
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
    let popup = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::unique(("completion", 0usize))))
        .expect("popup");
    let pos = popup.left_top() + egui::vec2(70.0, 17.0);
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
    assert_eq!(
        harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .len(),
        40
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
fn completion_after_at_only_offers_machines() {
    let registry = fixture_registry();
    let machine = registry
        .entry("assembling-machine-3")
        .unwrap()
        .glyph
        .to_string();
    let plate = registry.entry("iron-plate").unwrap().glyph.to_string();

    let inserts = suggested_ids(&registry, "@asm");
    assert!(inserts.contains(&machine), "{inserts:?}");

    let inserts = suggested_ids(&registry, "@ipl");
    assert!(!inserts.contains(&plate), "{inserts:?}");
}

#[test]
fn completion_fuzzy_matches_highlighted_characters() {
    let registry = fixture_registry();
    let cursor = "eec".chars().count();
    let (start, word) = completion_word("eec", cursor);
    let item = completion_items(&registry, "eec", start, &word)
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
        let items = completion_items(&registry, query, start, &word);
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
    let items = completion_items(&registry, "green-chip", start, &word);
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
    let items = completion_items(&registry, "green-chip", start, &word);
    let item = items
        .iter()
        .find(|item| item.label == "green-chip")
        .expect("alias row");
    assert_eq!(item.name.as_deref(), Some("Electronic circuit"));
    assert_eq!(item.matched, (0..10).collect::<Vec<_>>());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn completion_names_start_at_the_same_column() {
    let short = CompletionItem {
        insert: String::new(),
        glyph: 'a',
        label: "red-chip".to_string(),
        matched: Vec::new(),
        name: Some("Advanced circuit".to_string()),
        name_matched: Vec::new(),
    };
    let long = CompletionItem {
        label: "green-circuit".to_string(),
        ..short.clone()
    };
    let label_width = 13;
    let short_row = completion_row_text(&short, label_width);
    let long_row = completion_row_text(&long, label_width);
    assert_eq!(
        short_row.find("Advanced circuit"),
        long_row.find("Advanced circuit")
    );
    assert_eq!(
        short_row.find("Advanced circuit"),
        Some(label_width + COMPLETION_NAME_GAP + 2)
    );
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

    type_into_editor(&mut harness, " i");
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
