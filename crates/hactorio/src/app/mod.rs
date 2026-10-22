//! The document window: tabs, line editor with outputs, settings, breakdown.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Align, Context, Layout, RichText, TextStyle, Ui};
use hactorio_core::{BeltTier, Settings, StatementOutput, TimeUnit, from_plaintext, to_plaintext};
use hactorio_data::{GameInstall, Registry, locate};

use crate::document::{
    Cursor, DOCUMENT_VERSION, Document, MAX_FONT_SIZE, MAX_ZOOM, MIN_FONT_SIZE, MIN_ZOOM, Tab,
    Window,
};
use crate::evaluator::Evaluator;
use crate::icons;

mod completion;
mod detail;
mod editor;

use completion::CompletionState;
use editor::{EditorState, editor_ui};

const SAVE_INTERVAL: Duration = Duration::from_secs(5);

/// Font size of icon glyphs, such as belt tiers.
const ICON_SIZE: f32 = 18.0;

/// Sets the monospace text size globally, so the editor, gutter outputs, and popups agree.
fn set_font_size(ctx: &Context, size: f32) {
    ctx.all_styles_mut(|style| {
        style
            .text_styles
            .insert(TextStyle::Monospace, egui::FontId::monospace(size));
    });
}

/// The app's data directory, where the cache and document live.
pub fn data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join("Library/Application Support/hactorio");
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(data).join("hactorio");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local/share/hactorio");
        }
    }
    #[cfg(target_os = "windows")]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("hactorio");
    }
    PathBuf::from(".hactorio")
}

/// Height of a button whose text is an icon glyph.
fn icon_button_height(ui: &Ui) -> f32 {
    let font = egui::FontId::proportional(ICON_SIZE);
    let text_height = ui.fonts_mut(|fonts| fonts.row_height(&font));
    ui.spacing()
        .interact_size
        .y
        .max(text_height + 2.0 * ui.spacing().button_padding.y)
}

/// Formats the productivity a research grants at `level` for one recipe.
fn research_effect_label(
    registry: &Registry,
    effect: &hactorio_data::RecipeProductivity,
    level: u32,
) -> String {
    let percent = effect.change * f64::from(level) * 100.0;
    let percent = (percent * 1e6).round() / 1e6;
    let percent = format!("{percent}");
    let recipe = effect.recipe.as_ref();
    let name = registry
        .find_recipe(recipe)
        .and_then(|recipe| recipe.display_name.as_deref())
        .unwrap_or(recipe);
    match registry.entry(recipe) {
        Some(entry) => format!("+{percent}% {} {name}", entry.glyph),
        None => format!("+{percent}% {name}"),
    }
}

/// The game item whose icon represents a belt tier.
fn belt_tier_glyph(registry: &Registry, tier: BeltTier) -> Option<char> {
    registry.entry(tier.item_id()).map(|entry| entry.glyph)
}

pub struct HactorioApp {
    tabs: Vec<Tab>,
    active: usize,
    settings: Settings,
    evaluator: Evaluator,
    registry: Option<Arc<Registry>>,
    outputs: Vec<StatementOutput>,
    pending: u64,
    last_request: Option<(String, Settings)>,
    install: Option<GameInstall>,
    load: LoadState,
    status: String,
    dirty: bool,
    last_save: Instant,
    app_dir: PathBuf,
    initial_size: Option<[f32; 2]>,
    sized: bool,
    window: Option<Window>,
    results_width: f32,
    /// The GUI zoom factor currently shown, as saved in the document.
    zoom: f32,
    /// Monospace font size for the editor and outputs.
    font_size: f32,
    /// Cursor to restore on the first frame, as `(line, column)`.
    pending_focus: Option<(usize, usize)>,
    /// Generation whose results are currently displayed.
    applied: u64,
    completion: CompletionState,
}

enum LoadState {
    Idle,
    Running(Receiver<Result<(Registry, GameInstall), String>>),
}

impl HactorioApp {
    pub fn new(ctx: &egui::Context, document: Document, app_dir: PathBuf) -> Self {
        let mut tabs: Vec<Tab> = if document.tabs.is_empty() {
            vec![Tab::from_text("# Main\n")]
        } else {
            document
                .tabs
                .iter()
                .map(|text| Tab::from_text(text))
                .collect()
        };
        let active = document.active_tab.min(tabs.len() - 1);
        for (tab, cursor) in tabs.iter_mut().zip(document.cursors.iter()) {
            tab.cursor_line = cursor
                .line
                .min(tab.text.split('\n').count().saturating_sub(1));
            tab.scroll = cursor.scroll;
        }

        let cache_path = app_dir.join("cache.json");
        let (registry, status) = match Registry::load(&cache_path) {
            Ok(registry) => {
                let status = format!("{} prototypes", registry.entries().len());
                (Some(Arc::new(registry)), status)
            }
            Err(error) if cache_path.exists() => (None, format!("Game data failed: {error}")),
            Err(_) => (None, String::new()),
        };
        if let Some(registry) = &registry {
            for tab in &mut tabs {
                let converted = from_plaintext(&tab.text, registry);
                tab.set_text(&converted);
            }
        }

        let install = locate();
        if let (Some(registry), Some(install)) = (&registry, &install) {
            icons::install(ctx, Arc::clone(registry), &install.icon_source());
        }

        let zoom = document.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        ctx.set_zoom_factor(zoom);
        let font_size = document.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        set_font_size(ctx, font_size);

        HactorioApp {
            tabs,
            active,
            settings: document.settings,
            evaluator: Evaluator::new(),
            registry,
            outputs: Vec::new(),
            pending: 0,
            last_request: None,
            install,
            load: LoadState::Idle,
            status,
            dirty: false,
            last_save: Instant::now(),
            app_dir,
            initial_size: document.window.map(|window| [window.width, window.height]),
            sized: false,
            window: document.window,
            results_width: document.results_width,
            zoom,
            font_size,
            pending_focus: document
                .cursors
                .get(active)
                .map(|cursor| (cursor.line, cursor.column)),
            applied: 0,
            completion: CompletionState::default(),
        }
    }

    fn current_text(&self) -> String {
        self.tabs
            .get(self.active)
            .map(|tab| tab.text.clone())
            .unwrap_or_default()
    }

    fn poll_evaluator(&mut self) {
        while let Some(response) = self.evaluator.poll() {
            if response.generation == self.pending {
                self.outputs = response.outputs;
                self.applied = response.generation;
            }
        }
    }

    fn maybe_request(&mut self) {
        let Some(registry) = self.registry.clone() else {
            self.outputs.clear();
            return;
        };
        let text = self.current_text();
        let changed = self
            .last_request
            .as_ref()
            .is_none_or(|(last, last_settings)| last != &text || last_settings != &self.settings);
        if changed {
            self.pending = self
                .evaluator
                .request(text.clone(), self.settings.clone(), registry);
            self.last_request = Some((text, self.settings.clone()));
        }
    }

    fn loading(&self) -> bool {
        matches!(self.load, LoadState::Running(_))
    }

    fn start_load(&mut self) {
        if self.loading() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = match locate() {
                Some(install) => match install.dump() {
                    Ok(registry) => Ok((registry, install)),
                    Err(error) => Err(error.to_string()),
                },
                None => Err("Factorio was not found".to_string()),
            };
            let _ = tx.send(result);
        });
        self.load = LoadState::Running(rx);
        self.status = "Running data dumps…".to_string();
    }

    fn poll_load(&mut self, ctx: &Context) {
        let outcome = match &self.load {
            LoadState::Running(rx) => rx.try_recv().ok(),
            LoadState::Idle => None,
        };
        let Some(outcome) = outcome else {
            return;
        };
        self.load = LoadState::Idle;
        match outcome {
            Ok((registry, install)) => self.install_data(registry, install, ctx),
            Err(error) => self.status = error,
        }
    }

    fn install_data(&mut self, registry: Registry, install: GameInstall, ctx: &Context) {
        // Canonicalize to plaintext with the old table (when there is one),
        // then re-lex the whole document with the new table. This also runs on
        // the first load, so `:id:` tokens typed before any data was loaded
        // gain their glyphs immediately.
        for tab in &mut self.tabs {
            let plain = match &self.registry {
                Some(old) => to_plaintext(&tab.text, old),
                None => tab.text.clone(),
            };
            let relexed = from_plaintext(&plain, &registry);
            tab.set_text(&relexed);
        }

        let registry = Arc::new(registry);
        match registry.save(&self.app_dir.join("cache.json")) {
            Ok(()) => self.status = format!("{} prototypes", registry.entries().len()),
            Err(error) => self.status = format!("Could not cache game data: {error}"),
        }
        icons::install(ctx, Arc::clone(&registry), &install.icon_source());
        self.registry = Some(registry);
        self.install = Some(install);
        self.evaluator = Evaluator::new();
        self.last_request = None;
        self.outputs.clear();
        self.dirty = true;
    }

    fn tabs_bar(&mut self, ui: &mut Ui) {
        let mut activate = None;
        let mut close = None;
        ui.horizontal_wrapped(|ui| {
            for (index, tab) in self.tabs.iter().enumerate() {
                if ui
                    .selectable_label(index == self.active, tab.title())
                    .clicked()
                {
                    activate = Some(index);
                }
                if self.tabs.len() > 1 && ui.small_button("×").clicked() {
                    close = Some(index);
                }
            }
            if ui.button("+").clicked() {
                self.tabs.push(Tab::from_text("# Untitled\n"));
                self.active = self.tabs.len() - 1;
                self.dirty = true;
                self.last_request = None;
            }

            ui.separator();
            let loading = self.loading();
            let label = if loading {
                "Loading…".to_string()
            } else {
                "Reload game data".to_string()
            };
            if ui.add_enabled(!loading, egui::Button::new(label)).clicked() {
                self.start_load();
            }
        });

        if let Some(index) = activate
            && index != self.active
        {
            self.active = index;
            self.outputs.clear();
            self.last_request = None;
        }
        if let Some(index) = close {
            self.tabs.remove(index);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len() - 1;
            }
            self.dirty = true;
            self.last_request = None;
        }
    }

    fn display_bar(&mut self, ui: &mut Ui, registry: &Registry) {
        let before = self.settings.clone();
        let belt_icons = BeltTier::ALL.map(|tier| (tier, belt_tier_glyph(registry, tier)));

        ui.horizontal_wrapped(|ui| {
            // Reserve the tallest control's height so labels and controls align.
            ui.set_height(icon_button_height(ui));
            ui.strong("Display");
            // ui.add_space(8.0);
            ui.separator();

            ui.label("Font size");
            let font_size = ui.add(
                egui::Slider::new(&mut self.font_size, MIN_FONT_SIZE..=MAX_FONT_SIZE)
                    .step_by(1.0)
                    .fixed_decimals(0),
            );
            if font_size.changed() {
                set_font_size(ui.ctx(), self.font_size);
                self.dirty = true;
            }
            ui.separator();

            ui.label("Time unit");
            for unit in [TimeUnit::Second, TimeUnit::Minute, TimeUnit::Hour] {
                ui.selectable_value(&mut self.settings.time_unit, unit, unit.name());
            }
            ui.separator();

            ui.label("Item rates");
            ui.selectable_value(&mut self.settings.item_rate_belt, None, "item/s");
            for (tier, glyph) in belt_icons {
                let label: egui::WidgetText = match glyph {
                    Some(glyph) => RichText::new(glyph.to_string()).size(ICON_SIZE).into(),
                    None => tier.token().to_string().into(),
                };
                ui.selectable_value(&mut self.settings.item_rate_belt, Some(tier), label)
                    .on_hover_text(format!("{} item/s", tier.items_per_second()));
            }
            ui.add_enabled_ui(self.settings.item_rate_belt.is_some(), |ui| {
                let stack_label =
                    RichText::new(hactorio_data::STACK_SIZE_GLYPH.to_string()).size(ICON_SIZE);
                ui.toggle_value(&mut self.settings.stacked_belts, stack_label)
                    .on_hover_text("Show item rates as stacks on stacked belts");
            });
            ui.separator();

            ui.label("Decimals");
            ui.add(egui::Slider::new(&mut self.settings.max_decimals, 0..=6));
            ui.separator();

            ui.checkbox(&mut self.settings.metric_prefixes, "Metric prefixes");
        });

        self.settings_changed(before);
    }

    fn research_bar(&mut self, ui: &mut Ui, registry: &Registry) {
        let before = self.settings.clone();
        let researches = registry.productivity_researches();

        ui.horizontal_wrapped(|ui| {
            // Reserve the tallest control's height so labels and controls align.
            ui.set_height(icon_button_height(ui));
            ui.strong("Research");
            ui.separator();

            ui.label("Belt stacking");
            ui.add(egui::Slider::new(&mut self.settings.belt_stacking, 1..=4));
            ui.separator();

            ui.add_enabled_ui(!researches.is_empty(), |ui| {
                let measured_width = {
                    let mut measure = Ui::new(
                        ui.ctx().clone(),
                        ui.scope_id().with("productivity_menu_measure"),
                        egui::UiBuilder::new()
                            .layer_id(ui.layer_id())
                            .max_rect(egui::Rect::from_min_size(
                                ui.max_rect().min,
                                ui.ctx().content_rect().size(),
                            ))
                            .layout(egui::Layout::top_down(egui::Align::Min))
                            .style(ui.style().clone())
                            .invisible()
                            .sizing_pass(),
                    );
                    self.research_menu_contents(&mut measure, registry, researches);
                    measure.min_rect().width()
                };

                let button = ui
                    .button("Productivity…")
                    .on_hover_text("Set completed levels of productivity research");
                egui::Popup::menu(&button)
                    .layout(egui::Layout::top_down(egui::Align::Min))
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .style(egui::style::StyleModifier::new(|style| {
                        style.wrap_mode = Some(egui::TextWrapMode::Extend);
                    }))
                    .show(|ui| {
                        ui.set_width(measured_width.max(260.0));
                        let resized = self.research_menu_contents(ui, registry, researches);
                        if resized {
                            ui.ctx().request_repaint();
                        }
                    });
            });
        });

        self.settings_changed(before);
    }

    /// Lays out the productivity menu; returns whether a level changed.
    ///
    /// A sizing-pass Ui skips the full-width group constraint so the caller can
    /// measure the natural content width.
    fn research_menu_contents(
        &mut self,
        ui: &mut Ui,
        registry: &Registry,
        researches: &[hactorio_data::ProductivityResearch],
    ) -> bool {
        let sizing = ui.is_sizing_pass();
        let mut resized = false;
        for research in researches {
            ui.group(|ui| {
                if !sizing {
                    ui.set_min_width(ui.available_width());
                }
                let label = research.display_name.as_deref().unwrap_or(&research.id);
                let mut level = self.settings.research_level(&research.id);
                let drag = ui.horizontal(|ui| {
                    let drag = ui.add(
                        egui::DragValue::new(&mut level)
                            .range(0..=u32::MAX)
                            .speed(0.1),
                    );
                    ui.strong(label);
                    drag
                });
                if drag.inner.changed() {
                    self.settings
                        .research_levels
                        .insert(research.id.clone(), level);
                    resized = true;
                }
                for effect in &research.effects {
                    ui.label(research_effect_label(registry, effect, level));
                }
            });
        }
        resized
    }

    fn settings_changed(&mut self, before: Settings) {
        if self.settings != before {
            self.dirty = true;
            self.last_request = None;
        }
    }

    fn snapshot(&self) -> Document {
        Document {
            version: DOCUMENT_VERSION,
            tabs: self
                .tabs
                .iter()
                .map(|tab| match &self.registry {
                    Some(registry) => to_plaintext(&tab.text, registry),
                    None => tab.text.clone(),
                })
                .collect(),
            active_tab: self.active,
            cursors: self
                .tabs
                .iter()
                .map(|tab| Cursor {
                    line: tab.cursor_line,
                    column: 0,
                    scroll: tab.scroll,
                })
                .collect(),
            window: self.window,
            settings: self.settings.clone(),
            results_width: self.results_width,
            zoom: self.zoom,
            font_size: self.font_size,
        }
    }

    /// Marks the document dirty when the GUI zoom changed.
    fn sync_zoom(&mut self, ctx: &Context) {
        let zoom = ctx.zoom_factor();
        if zoom != self.zoom {
            self.zoom = zoom;
            self.dirty = true;
        }
    }

    fn save(&mut self) {
        match self.snapshot().save(&self.app_dir.join("document.json")) {
            Ok(()) => {
                self.dirty = false;
                self.last_save = Instant::now();
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }
}

impl HactorioApp {
    /// One frame of the whole app.
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.sync_zoom(&ctx);
        if !self.sized {
            if let Some(size) = self.initial_size {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size.into()));
            }
            self.sized = true;
        }

        let rect = ctx.viewport_rect();
        self.window = Some(Window {
            width: rect.width(),
            height: rect.height(),
            maximized: ctx.input(|input| input.viewport().maximized.unwrap_or(false)),
        });

        self.poll_load(&ctx);
        self.poll_evaluator();
        self.maybe_request();
        if self.applied < self.pending {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        let loaded = self.registry.clone();
        if let Some(registry) = &loaded {
            egui::Panel::top("tabs").show(ui, |ui| {
                self.tabs_bar(ui);
            });

            egui::Panel::top("display_settings").show(ui, |ui| {
                self.display_bar(ui, registry);
            });

            egui::Panel::top("research_settings").show(ui, |ui| {
                self.research_bar(ui, registry);
            });

            egui::Panel::bottom("status").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).small().weak());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let lines = self.tabs[self.active].text.split('\n').count();
                        ui.label(RichText::new(format!("{lines} lines")).small().weak());
                    });
                });
            });
        }

        egui::CentralPanel::default().show(ui, |ui| {
            let Some(registry) = self.registry.clone() else {
                ui.vertical_centered(|ui| {
                    ui.add_space(48.0);
                    ui.label("Game data is not loaded yet.");
                    let loading = self.loading();
                    let label = if loading {
                        "Loading…"
                    } else {
                        "Load game data"
                    };
                    if ui.add_enabled(!loading, egui::Button::new(label)).clicked() {
                        self.start_load();
                    }
                    if !self.status.is_empty() {
                        ui.label(RichText::new(&self.status).small().weak());
                    }
                });
                return;
            };
            let registry = registry.as_ref();
            let active = self.active;
            let mut state = EditorState::new(
                active,
                &self.outputs,
                &self.settings,
                registry,
                &mut self.pending_focus,
                &mut self.completion,
                &mut self.results_width,
            );
            let result = editor_ui(ui, &mut self.tabs[active], &mut state);
            if result.changed {
                self.dirty = true;
                self.last_request = None;
            }
            if result.resized {
                self.dirty = true;
            }
            let detail = detail::DetailContext {
                tabs: &self.tabs,
                active,
                outputs: &self.outputs,
                settings: &self.settings,
                registry,
            };
            if let Some((line, anchor, width)) = result.detail
                && self.completion.items.is_empty()
            {
                detail.detail_panel(&ctx, anchor, line, width);
            }
            if let Some((line, anchor)) = result.hover {
                detail.breakdown_popup(&ctx, line, anchor);
            }
        });

        if self.dirty && self.last_save.elapsed() >= SAVE_INTERVAL {
            self.save();
        }
    }
}

impl eframe::App for HactorioApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        HactorioApp::ui(self, ui);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        HactorioApp::save(self);
    }
}

#[cfg(test)]
mod tests;
