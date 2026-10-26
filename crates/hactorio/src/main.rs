mod app;
mod clipboard;
mod document;
mod evaluator;
mod fonts;
mod glyphs;

use eframe::egui;

const APP_TITLE: &str = "Hactar's Factorio Calculator";

fn main() -> eframe::Result {
    let app_dir = app::data_dir();
    let _ = std::fs::create_dir_all(&app_dir);

    let document = document::Document::load(&app_dir.join("document.json")).unwrap_or_default();
    let size = document
        .window
        .map(|window| [window.width.max(480.0), window.height.max(320.0)])
        .unwrap_or([1280.0, 800.0]);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_title(APP_TITLE),
        ..Default::default()
    };

    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::HactorioApp::new(
                &cc.egui_ctx,
                document,
                app_dir,
            )))
        }),
    )
}
