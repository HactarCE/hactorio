mod app;
mod document;
mod evaluator;
mod icons;

use eframe::egui;

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
            .with_title("hactorio"),
        ..Default::default()
    };

    eframe::run_native(
        "hactorio",
        options,
        Box::new(move |cc| Ok(Box::new(app::HactorioApp::new(cc, document, app_dir)))),
    )
}
