use super::*;
use egui_kittest::kittest::Queryable;
use hactorio_data::{GameInstall, postprocess};

fn install(app_dir: &std::path::Path) -> GameInstall {
    GameInstall {
        binary: app_dir.join("factorio"),
        data_dir: app_dir.join("data"),
        mod_dir: app_dir.join("mods"),
        script_output: app_dir.join("script-output"),
    }
}

fn registry_from(fixture: &str) -> Registry {
    postprocess(fixture, &hactorio_data::Locale::empty()).unwrap()
}

#[test]
fn without_game_data_only_the_load_prompt_is_shown() {
    let (mut harness, app_dir) = harness_with("no-data", sample_document(), None);
    harness.run();

    harness.get_by_label("Game data is not loaded yet.");
    harness.get_by_label("Load game data");
    assert!(
        harness
            .query_by_role(egui::Role::MultilineTextInput)
            .is_none(),
        "the editor must not be reachable before game data is loaded"
    );

    std::fs::remove_dir_all(app_dir).unwrap();
}

#[test]
fn loaded_game_data_offers_a_reload_button() {
    let (mut harness, app_dir) = harness_with("reload-label", sample_document(), Some(FIXTURE));
    harness.run();

    harness.get_by_label("Reload game data");
    assert!(harness.query_by_label("Load game data").is_none());
    assert!(
        harness
            .query_by_role(egui::Role::MultilineTextInput)
            .is_some()
    );

    std::fs::remove_dir_all(app_dir).unwrap();
}

#[test]
fn installing_data_lexes_plaintext_tokens_from_an_empty_registry() {
    let app_dir = std::env::temp_dir().join(format!("hactorio-first-load-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&app_dir).unwrap();

    let document = Document {
        tabs: vec!["# Main\n3 :iron-plate: / second\n".to_string()],
        ..Document::default()
    };
    let ctx = egui::Context::default();
    let mut app = HactorioApp::new(&ctx, document, app_dir.clone());
    assert!(app.registry.is_none());
    assert!(app.tabs[0].text.contains(":iron-plate:"));

    let registry = registry_from(FIXTURE);
    let glyph = registry.entry("iron-plate").unwrap().glyph;
    app.install_data(registry, install(&app_dir), &ctx);

    assert!(app.tabs[0].text.contains(glyph), "{:?}", app.tabs[0].text);
    assert!(!app.tabs[0].text.contains(":iron-plate:"));

    std::fs::remove_dir_all(app_dir).unwrap();
}

#[test]
fn reloading_data_relexes_glyphs_from_the_previous_table() {
    let app_dir = std::env::temp_dir().join(format!("hactorio-reload-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&app_dir);
    std::fs::create_dir_all(&app_dir).unwrap();

    let document = Document {
        tabs: vec!["# Main\n3 :iron-plate: / second\n".to_string()],
        ..Document::default()
    };
    let ctx = egui::Context::default();
    let mut app = HactorioApp::new(&ctx, document, app_dir.clone());

    let first = registry_from(FIXTURE);
    let first_glyph = first.entry("iron-plate").unwrap().glyph;
    app.install_data(first, install(&app_dir), &ctx);
    assert!(app.tabs[0].text.contains(first_glyph));

    // A new entry sorted before `iron-plate` shifts every later glyph.
    let shifted = FIXTURE.replace(
        "\"item\": {",
        "\"item\": {\n    \"aaa-first\": {\"type\": \"item\", \"name\": \"aaa-first\", \"stack_size\": 1},",
    );
    let second = registry_from(&shifted);
    let second_glyph = second.entry("iron-plate").unwrap().glyph;
    assert_ne!(first_glyph, second_glyph);

    app.install_data(second, install(&app_dir), &ctx);
    assert!(
        app.tabs[0].text.contains(second_glyph),
        "{:?}",
        app.tabs[0].text
    );
    assert!(!app.tabs[0].text.contains(first_glyph));
    assert!(!app.tabs[0].text.contains(":iron-plate:"));

    std::fs::remove_dir_all(app_dir).unwrap();
}
