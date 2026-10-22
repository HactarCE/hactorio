use super::*;

/// Renders the app offscreen and writes a snapshot.
///
/// Opt-in: set `HACTORIO_UI_SNAPSHOT=1` and run with `UPDATE_SNAPSHOTS=force`.
/// It needs a post-processed cache and the game's icon files.
#[test]
fn ui_snapshot() {
    if std::env::var_os("HACTORIO_UI_SNAPSHOT").is_none() {
        return;
    }
    let app_dir = data_dir();
    if !app_dir.join("cache.json").is_file() || locate().is_none() {
        eprintln!("skipping: no game data cache or Factorio install");
        return;
    }

    let document = sample_document();
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
    harness.snapshot("app");
}

/// Renders the completion popup for a typed `:iron`.
#[test]
fn completion_snapshot() {
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
    harness.snapshot("completion");
}
