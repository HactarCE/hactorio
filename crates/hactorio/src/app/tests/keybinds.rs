use super::*;
use egui::{Key, Modifiers};

#[test]
fn typing_an_open_paren_wraps_the_selection() {
    let document = editor_document("# Main\nab\n", 1, 2);
    let (mut harness, dir) = completion_harness("wrap-paren", document);
    harness.run();

    harness.key_press_modifiers(Modifiers::SHIFT, Key::ArrowLeft);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::ArrowLeft);
    harness.run();

    type_into_editor(&mut harness, "(");

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Main\n(ab)\n");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn typing_an_open_paren_without_a_selection_inserts_it() {
    let document = editor_document("# Main\nab\n", 1, 2);
    let (mut harness, dir) = completion_harness("plain-paren", document);
    harness.run();

    type_into_editor(&mut harness, "(");

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, "# Main\nab(\n");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ctrl_space_opens_completion_with_an_empty_query() {
    let document = editor_document("# Main\n", 1, 0);
    let (mut harness, dir) = completion_harness("ctrl-space-empty", document);
    harness.run();

    harness.key_press_modifiers(Modifiers::CTRL, Key::Space);
    harness.run();

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.completion.word, "");
    assert!(!app.completion.items.is_empty());
    assert_eq!(app.completion.selected, 0);
    assert!(
        harness
            .ctx
            .memory(|memory| memory.area_rect(egui::Id::unique(("completion", 0usize))))
            .is_some()
    );

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ctrl_space_with_an_empty_query_offers_variables() {
    let document = editor_document("# Main\nspeed = 3\n", 2, 0);
    let (mut harness, dir) = completion_harness("ctrl-space-variable", document);
    harness.run();
    harness.key_press_modifiers(Modifiers::CTRL, Key::Space);
    harness.run();

    let app = harness.state().as_ref().expect("app");
    assert!(
        app.completion
            .items
            .iter()
            .any(|item| item.insert == "speed"),
        "{:?}",
        app.completion
            .items
            .iter()
            .map(|item| &item.label)
            .collect::<Vec<_>>()
    );

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ctrl_space_again_leaves_the_open_popup_unchanged() {
    let document = editor_document("# Main\n", 1, 0);
    let (mut harness, dir) = completion_harness("ctrl-space-repeat", document);
    harness.run();
    harness.key_press_modifiers(Modifiers::CTRL, Key::Space);
    harness.run();
    harness.key_press(Key::ArrowDown);
    harness.run();

    let selected = harness.state().as_ref().expect("app").completion.selected;
    assert_eq!(selected, 1);

    harness.key_press_modifiers(Modifiers::CTRL, Key::Space);
    harness.run();

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.completion.selected, selected);
    assert!(!app.completion.items.is_empty());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn ctrl_space_reopens_a_dismissed_popup() {
    let document = editor_document("# Main\n", 1, 0);
    let (mut harness, dir) = completion_harness("ctrl-space-reopen", document);
    harness.run();
    harness.key_press_modifiers(Modifiers::CTRL, Key::Space);
    harness.run();
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

    harness.key_press_modifiers(Modifiers::CTRL, Key::Space);
    harness.run();
    assert!(
        !harness
            .state()
            .as_ref()
            .expect("app")
            .completion
            .items
            .is_empty()
    );

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn accepting_an_empty_query_completion_inserts_it() {
    let document = editor_document("# Main\n", 1, 0);
    let (mut harness, dir) = completion_harness("ctrl-space-accept", document);
    harness.run();
    harness.key_press_modifiers(Modifiers::CTRL, Key::Space);
    harness.run();
    let insert = harness
        .state()
        .as_ref()
        .expect("app")
        .completion
        .items
        .first()
        .expect("completions")
        .insert
        .clone();

    harness.key_press(Key::Enter);
    harness.run();

    let app = harness.state().as_ref().expect("app");
    assert_eq!(app.tabs[0].text, format!("# Main\n{insert}"));
    assert!(app.completion.items.is_empty());

    std::fs::remove_dir_all(dir).unwrap();
}
