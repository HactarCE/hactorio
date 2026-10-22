use super::*;

#[test]
fn save_and_load_convert_between_plaintext_and_glyphs() {
    let dir = std::env::temp_dir().join(format!("hactorio-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let registry = fixture_registry();
    registry.save(&dir.join("cache.json")).unwrap();
    let document = Document {
        tabs: vec!["# Main\n3 :iron-plate: / second\n".to_string()],
        ..Document::default()
    };
    let mut app = HactorioApp::new(&egui::Context::default(), document, dir.clone());

    let glyph = app
        .registry
        .as_ref()
        .unwrap()
        .entry("iron-plate")
        .unwrap()
        .glyph;
    assert!(app.tabs[0].text.contains(glyph), "{:?}", app.tabs[0].text);
    assert!(!app.tabs[0].text.contains(":iron-plate:"));

    app.save();
    let saved = std::fs::read_to_string(dir.join("document.json")).unwrap();
    assert!(saved.contains(":iron-plate:"), "{saved}");
    assert!(saved.contains("results_width"), "{saved}");
    assert!(!saved.contains(glyph), "{saved}");

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn font_size_is_restored_and_saved() {
    let dir = std::env::temp_dir().join(format!("hactorio-font-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let ctx = egui::Context::default();
    let mut app = HactorioApp::new(
        &ctx,
        Document {
            font_size: 18.0,
            ..Document::default()
        },
        dir.clone(),
    );
    assert_eq!(app.font_size, 18.0);
    assert_eq!(
        ctx.style_of(ctx.theme()).text_styles[&TextStyle::Monospace].size,
        18.0
    );

    app.save();
    let saved = Document::load(&dir.join("document.json")).unwrap();
    assert_eq!(saved.font_size, 18.0);

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn zoom_is_restored_and_saved() {
    let dir = std::env::temp_dir().join(format!("hactorio-zoom-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let ctx = egui::Context::default();
    let document = Document {
        zoom: 1.4,
        ..Document::default()
    };
    let mut app = HactorioApp::new(&ctx, document, dir.clone());
    assert_eq!(app.zoom, 1.4);

    ctx.run_ui(egui::RawInput::default(), |ui| app.ui(ui))
        .drop_without_applying_deltas();
    assert_eq!(ctx.zoom_factor(), 1.4);
    assert!(!app.dirty);

    ctx.set_zoom_factor(0.8);
    ctx.run_ui(egui::RawInput::default(), |ui| app.ui(ui))
        .drop_without_applying_deltas();
    assert_eq!(app.zoom, 0.8);
    assert!(app.dirty);

    app.save();
    let saved = Document::load(&dir.join("document.json")).unwrap();
    assert_eq!(saved.zoom, 0.8);

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn font_family_is_restored_and_saved() {
    let Some(family) = crate::fonts::system_fonts().families().first().cloned() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("hactorio-font-family-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let ctx = egui::Context::default();
    let mut app = HactorioApp::new(
        &ctx,
        Document {
            font: Some(family.clone()),
            ..Document::default()
        },
        dir.clone(),
    );
    assert_eq!(app.font, Some(family.clone()));

    app.save();
    let saved = Document::load(&dir.join("document.json")).unwrap();
    assert_eq!(saved.font, Some(family));

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unknown_font_family_falls_back_to_the_default() {
    let dir = std::env::temp_dir().join(format!("hactorio-font-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let ctx = egui::Context::default();
    let app = HactorioApp::new(
        &ctx,
        Document {
            font: Some("Definitely Not An Installed Font".to_string()),
            ..Document::default()
        },
        dir.clone(),
    );
    assert_eq!(app.font, None);

    std::fs::remove_dir_all(dir).unwrap();
}
