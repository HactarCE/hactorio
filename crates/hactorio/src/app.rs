//! The document window: tabs, line editor with outputs, settings, breakdown.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{
    Align, Color32, Context, CursorIcon, Key, Layout, Modifiers, Rect, RichText, ScrollArea, Sense,
    TextEdit, TextStyle, Ui,
};
use hactorio_core::{
    BaseUnit, BeltTier, LineOutput, Output, RecipeQuantity, Settings, StatementOutput, TimeUnit,
    Value, breakdown, format_amount, format_energy, format_output, format_power, format_value,
    from_plaintext, line_hash, to_plaintext,
};
use hactorio_data::{EntryId, GameInstall, Registry, locate};

use crate::document::{
    Cursor, DOCUMENT_VERSION, Document, MAX_FONT_SIZE, MAX_ZOOM, MIN_FONT_SIZE, MIN_ZOOM, Tab,
    Window,
};
use crate::evaluator::Evaluator;
use crate::icons;

const SAVE_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum height of the autocomplete popup, before it scrolls.
const COMPLETION_MAX_HEIGHT: f32 = 240.0;

/// Font size of icon glyphs, such as belt tiers.
const ICON_SIZE: f32 = 18.0;

/// Frame chrome between the detail popup's outer corner and its content.
fn detail_popup_inset(style: &egui::Style) -> egui::Vec2 {
    let frame = egui::Frame::popup(style);
    frame.inner_margin.left_top() + egui::vec2(frame.stroke.width, frame.stroke.width)
}

/// Where to place the detail popup so its content starts at `anchor`.
fn detail_popup_pos(
    style: &egui::Style,
    anchor: egui::Pos2,
    screen: egui::Rect,
    width: f32,
) -> egui::Pos2 {
    let inset = detail_popup_inset(style);
    egui::pos2(
        (anchor.x - inset.x)
            .min(screen.right() - width)
            .max(screen.left() + 8.0),
        (anchor.y - inset.y)
            .min(screen.bottom() - 160.0)
            .max(screen.top() + 8.0),
    )
}

/// The icon glyph under `pointer` in a laid-out text, with its row and rect.
fn hovered_icon(
    registry: &Registry,
    galley: &egui::Galley,
    origin: egui::Pos2,
    pointer: egui::Pos2,
) -> Option<(usize, egui::Rect)> {
    for (row_index, row) in galley.rows.iter().enumerate() {
        let top = origin.y + row.pos.y;
        if pointer.y < top || pointer.y > top + row.size.y {
            continue;
        }
        for glyph in &row.glyphs {
            if registry.entry_by_glyph(glyph.chr).is_none() {
                continue;
            }
            let rect = egui::Rect::from_min_size(
                egui::pos2(origin.x + row.pos.x + glyph.pos.x, top),
                egui::vec2(glyph.advance_width, row.size.y),
            );
            if rect.contains(pointer) {
                return Some((row_index, rect));
            }
        }
    }
    None
}

/// Whether a popup is covering `pointer`, so editors below it do not react.
fn pointer_over_popup(ctx: &Context, active: usize, pointer: egui::Pos2) -> bool {
    [
        egui::Id::unique("line_detail"),
        egui::Id::unique(("completion", active)),
    ]
    .into_iter()
    .any(|id| {
        let layer = egui::LayerId::new(egui::Order::Foreground, id);
        ctx.memory(|memory| {
            memory.areas().visible_last_frame(&layer)
                && memory
                    .area_rect(id)
                    .is_some_and(|rect| rect.contains(pointer))
        })
    })
}

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

/// The game item whose icon represents a belt tier.
fn belt_tier_glyph(registry: &Registry, tier: BeltTier) -> Option<char> {
    let id = match tier {
        BeltTier::Yellow => "transport-belt",
        BeltTier::Red => "fast-transport-belt",
        BeltTier::Blue => "express-transport-belt",
        BeltTier::Green => "turbo-transport-belt",
    };
    registry.entry(id).map(|entry| entry.glyph)
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
            Err(_) => (
                None,
                "No game data yet; use \"Load game data\".".to_string(),
            ),
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

    fn start_load(&mut self) {
        if matches!(self.load, LoadState::Running(_)) {
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
        if let Some(old) = &self.registry {
            for tab in &mut self.tabs {
                let plain = to_plaintext(&tab.text, old);
                let relexed = from_plaintext(&plain, &registry);
                tab.set_text(&relexed);
            }
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
            let loading = matches!(self.load, LoadState::Running(_));
            let label = if loading {
                "Loading…".to_string()
            } else {
                "Load game data".to_string()
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

    fn display_bar(&mut self, ui: &mut Ui) {
        let before = self.settings.clone();
        let belt_icons = BeltTier::ALL.map(|tier| {
            let glyph = self
                .registry
                .as_deref()
                .and_then(|registry| belt_tier_glyph(registry, tier));
            (tier, glyph)
        });
        let has_icons = self.registry.is_some();

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
                    .on_hover_text(format!("{} belts", tier.name()));
            }
            ui.add_enabled_ui(self.settings.item_rate_belt.is_some(), |ui| {
                let stack_label = if has_icons {
                    RichText::new(icons::STACK_SIZE.to_string()).size(ICON_SIZE)
                } else {
                    RichText::new("stacked")
                };
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

    fn research_bar(&mut self, ui: &mut Ui) {
        let before = self.settings.clone();

        ui.horizontal_wrapped(|ui| {
            // Reserve the tallest control's height so labels and controls align.
            ui.set_height(icon_button_height(ui));
            ui.strong("Research");
            ui.separator();

            ui.label("Belt stacking");
            ui.add(egui::Slider::new(&mut self.settings.belt_stacking, 1..=4));
        });

        self.settings_changed(before);
    }

    fn settings_changed(&mut self, before: Settings) {
        if self.settings != before {
            self.dirty = true;
            self.last_request = None;
        }
    }

    fn breakdown_ui(&self, ui: &mut Ui, line: usize) {
        let (Some(registry), Some(tab)) = (&self.registry, self.tabs.get(self.active)) else {
            return;
        };
        let (Some(text), Some(output)) = (tab.text.split('\n').nth(line), self.outputs.get(line))
        else {
            return;
        };
        if output.source_hash != line_hash(text) {
            return;
        }
        let Some(value) = &output.eval else {
            return;
        };

        ui.heading("Breakdown");
        let detail = breakdown(value, &self.settings, registry);
        for section in detail.sections {
            ui.add_space(6.0);
            let title = section.title.clone();
            ui.label(RichText::new(&title).strong());
            egui::Grid::new(("breakdown", &title))
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for row in section.rows {
                        ui.label(row.label);
                        ui.label(row.value);
                        ui.end_row();
                    }
                });
        }
    }

    /// The breakdown for `line`, shown next to a hovered icon.
    fn breakdown_popup(&self, ctx: &Context, line: usize, anchor: egui::Rect) {
        let (Some(tab), Some(output)) = (self.tabs.get(self.active), self.outputs.get(line)) else {
            return;
        };
        let Some(text) = tab.text.split('\n').nth(line) else {
            return;
        };
        if output.source_hash != line_hash(text) || output.eval.is_none() {
            return;
        }

        let pointer = ctx.pointer_hover_pos().unwrap_or(anchor.center());
        let layer = ctx.layer_id_at(pointer).unwrap_or_else(|| {
            egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("breakdown"))
        });
        egui::Tooltip::always_open(
            ctx.clone(),
            layer,
            egui::Id::unique(("breakdown", line)),
            anchor,
        )
        .width(360.0)
        .show(|ui| self.breakdown_ui(ui, line));
    }

    /// Full results for the line under the caret, floating beside it.
    fn detail_panel(&self, ctx: &Context, anchor: egui::Pos2, line: usize, width: f32) {
        let (Some(registry), Some(tab)) = (&self.registry, self.tabs.get(self.active)) else {
            return;
        };
        let Some(text) = tab.text.split('\n').nth(line) else {
            return;
        };
        let Some(statement) = self.outputs.get(line) else {
            return;
        };
        if statement.source_hash != line_hash(text) {
            return;
        }

        let screen = ctx.viewport_rect();
        let width = width.clamp(240.0, 420.0);
        let style = ctx.style_of(ctx.theme());
        let pos = detail_popup_pos(&style, anchor, screen, width);
        let hovered = egui::Area::new(egui::Id::unique("line_detail"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        ui.set_min_width((width - 20.0).max(240.0));
                        ui.set_max_width(width);
                        detail_ui(ui, statement, &self.settings, registry)
                    })
                    .inner
            })
            .inner;
        if let Some(anchor) = hovered {
            self.breakdown_popup(ctx, line, anchor);
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
            x: rect.min.x,
            y: rect.min.y,
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

        egui::Panel::top("tabs").show(ui, |ui| {
            self.tabs_bar(ui);
        });

        egui::Panel::top("display_settings").show(ui, |ui| {
            self.display_bar(ui);
        });

        egui::Panel::top("research_settings").show(ui, |ui| {
            self.research_bar(ui);
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

        egui::CentralPanel::default().show(ui, |ui| {
            if self.registry.is_none() {
                ui.add_space(12.0);
                ui.heading("hactorio");
                ui.label("Game data is not loaded yet.");
                if ui.button("Load game data").clicked() {
                    self.start_load();
                }
                ui.separator();
            }
            let active = self.active;
            let mut state = EditorState {
                active,
                outputs: &self.outputs,
                settings: &self.settings,
                registry: self.registry.as_deref(),
                pending_focus: &mut self.pending_focus,
                completion: &mut self.completion,
                results_width: &mut self.results_width,
            };
            let result = editor_ui(ui, &mut self.tabs[active], &mut state);
            if result.changed {
                self.dirty = true;
                self.last_request = None;
            }
            if result.resized {
                self.dirty = true;
            }
            if let Some((line, anchor, width)) = result.detail
                && self.completion.items.is_empty()
            {
                self.detail_panel(&ctx, anchor, line, width);
            }
            if let Some((line, anchor)) = result.hover {
                self.breakdown_popup(&ctx, line, anchor);
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

struct EditorState<'a> {
    active: usize,
    outputs: &'a [StatementOutput],
    settings: &'a Settings,
    registry: Option<&'a Registry>,
    pending_focus: &'a mut Option<(usize, usize)>,
    completion: &'a mut CompletionState,
    results_width: &'a mut f32,
}

struct EditorResult {
    changed: bool,
    resized: bool,
    /// The line the caret is on, while the editor has focus.
    focused: Option<usize>,
    /// Where to place the full-results panel for the focused line, and its width.
    detail: Option<(usize, egui::Pos2, f32)>,
    /// The line whose icon is hovered, and the icon's rect.
    hover: Option<(usize, egui::Rect)>,
}

/// Renders the tab as one multiline editor, with line numbers and outputs
/// painted at the galley's row positions so they stay aligned.
fn editor_ui(ui: &mut Ui, tab: &mut Tab, state: &mut EditorState<'_>) -> EditorResult {
    let active = state.active;
    let outputs = state.outputs;
    let settings = state.settings;
    let registry = state.registry;
    let pending_focus: &mut Option<(usize, usize)> = state.pending_focus;
    let completion: &mut CompletionState = state.completion;
    let results_width: &mut f32 = state.results_width;

    let mut result = EditorResult {
        changed: false,
        resized: false,
        focused: None,
        detail: None,
        hover: None,
    };

    let font_id = TextStyle::Monospace.resolve(ui.style());
    let char_width = ui
        .fonts_mut(|fonts| fonts.glyph_width(&font_id, 'M'))
        .max(1.0);
    let line_count = tab.text.split('\n').count();
    let longest = tab
        .text
        .split('\n')
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let number_width = 44.0;
    let divider_width = 7.0;
    let central_width = ui.available_width();
    let max_results = (central_width - number_width - 220.0).max(120.0);
    let requested_results = (*results_width).clamp(120.0, max_results);
    let editor_width =
        (central_width - number_width - requested_results - divider_width).max(160.0);
    let editor_content_width = (longest as f32 * char_width + 24.0).max(editor_width);

    let mut layouter = |ui: &Ui, buffer: &dyn egui::TextBuffer, _wrap_width: f32| {
        let mut job = egui::text::LayoutJob::simple(
            buffer.as_str().to_string(),
            font_id.clone(),
            ui.visuals().text_color(),
            f32::INFINITY,
        );
        job.wrap.max_width = f32::INFINITY;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    };

    let scroll = ScrollArea::vertical()
        .id_salt(("editor", active))
        .vertical_scroll_offset(tab.scroll)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add_space(number_width);
                let popup_open = !completion.items.is_empty()
                    && completion.dismissed.as_deref() != Some(completion.word.as_str());
                let mut accept = false;
                let mut tab_accept = false;
                let mut dismiss = false;
                let mut move_selection = 0i32;
                if popup_open {
                    ui.input_mut(|input| {
                        let mut down = 0;
                        let mut up = 0;
                        while input.consume_key(Modifiers::NONE, Key::ArrowDown) {
                            down += 1;
                        }
                        while input.consume_key(Modifiers::NONE, Key::ArrowUp) {
                            up += 1;
                        }
                        tab_accept = consume_unmodified_key(input, Key::Tab);
                        accept = input.consume_key(Modifiers::NONE, Key::Enter) || tab_accept;
                        dismiss = input.consume_key(Modifiers::NONE, Key::Escape);
                        move_selection = down - up;
                    });
                    if tab_accept {
                        // Keep Tab from moving focus; it accepts the suggestion instead.
                        ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
                    }
                }

                let text_id = ui.make_persistent_id(("text", active));
                // Clicking the popup must not surrender the editor's focus, or the
                // popup would be cleared before the click reaches its row.
                let popup_id = egui::Id::unique(("completion", active));
                let pointer_on_popup = popup_open
                    && ui.ctx().pointer_interact_pos().is_some_and(|pos| {
                        ui.ctx()
                            .memory(|memory| memory.area_rect(popup_id))
                            .is_some_and(|rect| rect.contains(pos))
                    });
                let surrender_focus_on = ui
                    .ctx()
                    .options(|options| options.input_options.surrender_focus_on);
                if pointer_on_popup {
                    ui.ctx().options_mut(|options| {
                        options.input_options.surrender_focus_on = egui::SurrenderFocusOn::Never;
                    });
                }
                let hscroll = ScrollArea::horizontal()
                    .id_salt(("text_scroll", active))
                    .max_width(editor_width)
                    .show(ui, |ui| {
                        TextEdit::multiline(&mut tab.text)
                            .code_editor()
                            .event_filter(egui::EventFilter {
                                tab: false,
                                horizontal_arrows: true,
                                vertical_arrows: true,
                                escape: true,
                            })
                            .desired_width(editor_content_width)
                            .desired_rows(line_count.max(6))
                            .margin(egui::Margin::symmetric(4, 4))
                            .layouter(&mut layouter)
                            .id(text_id)
                            .show(ui)
                    });
                ui.ctx().options_mut(|options| {
                    options.input_options.surrender_focus_on = surrender_focus_on;
                });
                let text_edit = hscroll.inner;
                result.changed |= text_edit.response.changed();

                if let Some(registry) = registry
                    && let Some(pointer) = ui.ctx().pointer_hover_pos()
                    && text_edit.response.rect.contains(pointer)
                    && !pointer_over_popup(ui.ctx(), active, pointer)
                    && let Some((line, rect)) =
                        hovered_icon(registry, &text_edit.galley, text_edit.galley_pos, pointer)
                {
                    result.hover = Some((line, rect));
                }

                if let Some((line, column)) = *pending_focus {
                    let start: usize = tab
                        .text
                        .split('\n')
                        .take(line)
                        .map(|text| text.chars().count() + 1)
                        .sum();
                    let length = tab
                        .text
                        .split('\n')
                        .nth(line)
                        .map(|text| text.chars().count())
                        .unwrap_or(0);
                    let index = start + column.min(length);
                    let stored = egui::text_edit::TextEditState::load(ui.ctx(), text_id);
                    if let Some(mut state) = stored {
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(
                                egui::text::CCursor::new(index),
                            )));
                        state.store(ui.ctx(), text_id);
                        *pending_focus = None;
                    }
                    ui.memory_mut(|memory| memory.request_focus(text_id));
                }

                if let Some(range) = text_edit.cursor_range {
                    let line = tab
                        .text
                        .chars()
                        .take(range.primary.index.0)
                        .filter(|ch| *ch == '\n')
                        .count();
                    tab.cursor_line = line;
                    if text_edit.response.has_focus() {
                        result.focused = Some(line);
                    }
                }

                let cursor_char = text_edit
                    .cursor_range
                    .map(|range| range.primary.index.0)
                    .unwrap_or(0);
                let (word_start, word) = completion_word(&tab.text, cursor_char);
                let typed = text_edit.response.changed();
                if !text_edit.response.has_focus() {
                    completion.items.clear();
                } else if popup_open && !typed && cursor_char != completion.cursor {
                    // The caret moved without typing: close the popup.
                    completion.items.clear();
                }

                if let Some(registry) = registry
                    && typed
                    && text_edit.response.has_focus()
                {
                    completion.word = word.clone();
                    completion.selected = 0;
                    completion.needs_scroll = true;
                    completion.items = if word.is_empty() {
                        Vec::new()
                    } else {
                        completion_items(registry, &tab.text, word_start, &word)
                    };
                    if completion.dismissed.as_deref() != Some(word.as_str()) {
                        completion.dismissed = None;
                    }
                }
                completion.cursor = cursor_char;

                if popup_open && !completion.items.is_empty() {
                    if dismiss {
                        completion.dismissed = Some(completion.word.clone());
                        completion.items.clear();
                    } else {
                        if move_selection != 0 {
                            let len = completion.items.len() as i32;
                            completion.selected = (completion.selected as i32 + move_selection)
                                .rem_euclid(len)
                                as usize;
                            completion.needs_scroll = true;
                        }
                        if accept {
                            let inserted = insert_completion(
                                &mut tab.text,
                                word_start,
                                completion.word.chars().count(),
                                &completion.items[completion.selected].insert,
                            );
                            if let Some(mut state) =
                                egui::text_edit::TextEditState::load(ui.ctx(), text_id)
                            {
                                state
                                    .cursor
                                    .set_char_range(Some(egui::text::CCursorRange::one(
                                        egui::text::CCursor::new(inserted),
                                    )));
                                state.store(ui.ctx(), text_id);
                            }
                            ui.memory_mut(|memory| memory.request_focus(text_id));
                            completion.items.clear();
                            completion.word.clear();
                            result.changed = true;
                        } else {
                            let caret = text_edit
                                .galley
                                .pos_from_cursor(egui::text::CCursor::new(cursor_char));
                            let anchor = text_edit.galley_pos
                                + caret.left_bottom().to_vec2()
                                + egui::vec2(0.0, 2.0);
                            let screen = ui.ctx().viewport_rect();
                            let pos = egui::pos2(
                                anchor
                                    .x
                                    .min(screen.right() - 340.0)
                                    .max(screen.left() + 8.0),
                                anchor
                                    .y
                                    .min(screen.bottom() - 100.0)
                                    .max(screen.top() + 8.0),
                            );
                            let mut clicked = None;
                            egui::Area::new(egui::Id::unique(("completion", active)))
                                .order(egui::Order::Foreground)
                                .fixed_pos(pos)
                                .show(ui.ctx(), |ui| {
                                    let style = ui.style();
                                    let font_id = TextStyle::Monospace.resolve(style);
                                    let label_width = completion
                                        .items
                                        .iter()
                                        .map(|item| item.label.chars().count())
                                        .max()
                                        .unwrap_or(0);
                                    let text_width = completion
                                        .items
                                        .iter()
                                        .map(|item| {
                                            ui.fonts_mut(|fonts| {
                                                fonts
                                                    .layout_no_wrap(
                                                        completion_row_text(item, label_width),
                                                        font_id.clone(),
                                                        Color32::WHITE,
                                                    )
                                                    .size()
                                                    .x
                                            })
                                        })
                                        .fold(0.0, f32::max);
                                    let chrome = egui::Frame::popup(style);
                                    let chrome_width =
                                        chrome.inner_margin.sum().x + 2.0 * chrome.stroke.width;
                                    let content_width = (text_width
                                        + 2.0 * style.spacing.button_padding.x
                                        + style.spacing.scroll.allocated_width())
                                    .max(300.0);
                                    let total_width = (content_width + chrome_width)
                                        .min(ui.ctx().viewport_rect().width() - 16.0);
                                    let content_width = (total_width - chrome_width).max(100.0);

                                    // The area is sized by its contents, and its remembered size
                                    // would otherwise cap this frame's available size.
                                    ui.set_max_width(total_width);
                                    ui.set_max_height(COMPLETION_MAX_HEIGHT);
                                    let needs_scroll = completion.needs_scroll;
                                    let selected = completion.selected;
                                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                                        ui.set_min_width(content_width);
                                        ui.set_max_width(content_width);
                                        egui::ScrollArea::vertical()
                                            .id_salt(("completion_scroll", active))
                                            .max_height(COMPLETION_MAX_HEIGHT)
                                            .show(ui, |ui| {
                                                for (index, item) in
                                                    completion.items.iter().enumerate()
                                                {
                                                    let job = completion_layout_job(
                                                        item,
                                                        label_width,
                                                        index == selected,
                                                        &font_id,
                                                        ui.visuals(),
                                                    );
                                                    let response =
                                                        ui.selectable_label(index == selected, job);
                                                    if response.clicked() {
                                                        clicked = Some(index);
                                                    }
                                                    #[cfg(test)]
                                                    if index == selected {
                                                        completion.selected_rect =
                                                            Some(response.rect);
                                                    }
                                                    if needs_scroll && index == selected {
                                                        ui.scroll_to_rect_animation(
                                                            response.rect,
                                                            Some(egui::Align::Center),
                                                            egui::style::ScrollAnimation::none(),
                                                        );
                                                    }
                                                }
                                            });
                                    });
                                    if needs_scroll {
                                        completion.needs_scroll = false;
                                    }
                                });
                            if let Some(index) = clicked {
                                let inserted = insert_completion(
                                    &mut tab.text,
                                    word_start,
                                    completion.word.chars().count(),
                                    &completion.items[index].insert,
                                );
                                if let Some(mut state) =
                                    egui::text_edit::TextEditState::load(ui.ctx(), text_id)
                                {
                                    state.cursor.set_char_range(Some(
                                        egui::text::CCursorRange::one(egui::text::CCursor::new(
                                            inserted,
                                        )),
                                    ));
                                    state.store(ui.ctx(), text_id);
                                }
                                ui.memory_mut(|memory| memory.request_focus(text_id));
                                completion.items.clear();
                                completion.word.clear();
                                result.changed = true;
                            }
                        }
                    }
                }

                let viewport = hscroll.inner_rect;
                let origin = text_edit.galley_pos;
                let galley = &text_edit.galley;
                let editor_height = text_edit.response.rect.height().max(galley.size().y + 8.0);

                let (divider_rect, divider) =
                    ui.allocate_exact_size(egui::vec2(divider_width, editor_height), Sense::drag());
                if divider.hovered() || divider.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                if divider.dragged() {
                    *results_width =
                        (*results_width - divider.drag_delta().x).clamp(120.0, max_results);
                    result.resized = true;
                }
                let shown_results = (*results_width).clamp(120.0, max_results);
                ui.painter().vline(
                    divider_rect.center().x,
                    divider_rect.y_range(),
                    ui.visuals().widgets.noninteractive.bg_stroke,
                );
                let results_rect = Rect::from_min_size(
                    egui::pos2(divider_rect.right(), divider_rect.top()),
                    egui::vec2(shown_results, editor_height),
                );
                ui.allocate_space(egui::vec2(shown_results, editor_height));

                let gutter_x = results_rect.left() + 8.0;
                let gutter_room = (shown_results - 16.0).max(60.0);
                let weak = ui.visuals().weak_text_color();
                let error_color = Color32::from_rgb(220, 90, 90);
                let painter = ui.painter();

                for (index, line) in tab.text.split('\n').enumerate() {
                    let Some(row) = galley.rows.get(index) else {
                        break;
                    };
                    let y = origin.y + row.pos.y;
                    painter.text(
                        egui::pos2(viewport.left() - 10.0, y),
                        egui::Align2::RIGHT_TOP,
                        format!("{}", index + 1),
                        font_id.clone(),
                        weak,
                    );

                    let (Some(statement), Some(registry)) = (outputs.get(index), registry) else {
                        continue;
                    };
                    if statement.source_hash != line_hash(line) {
                        continue;
                    }
                    let Some(text) = output_text(statement, settings, registry) else {
                        continue;
                    };
                    if result.focused == Some(index) {
                        result.detail = Some((index, egui::pos2(gutter_x, y), gutter_room));
                        continue;
                    }
                    let color = if matches!(statement.output, LineOutput::Error(_)) {
                        error_color
                    } else {
                        weak
                    };
                    let text = truncate_text(painter, &text, &font_id, gutter_room);
                    let pos = egui::pos2(gutter_x, y);
                    let galley = painter.layout_no_wrap(text, font_id.clone(), color);
                    painter.galley(pos, galley.clone(), color);
                    if let Some(pointer) = ui.ctx().pointer_hover_pos()
                        && !pointer_over_popup(ui.ctx(), active, pointer)
                        && let Some((_, rect)) = hovered_icon(registry, &galley, pos, pointer)
                    {
                        result.hover = Some((index, rect));
                    }
                }
            });
        });
    tab.scroll = scroll.state.clamped_offset().y;

    result
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionContext {
    Normal,
    Machine,
    Recipe,
}

#[derive(Clone, Default)]
struct CompletionState {
    /// Character index of the caret when completions were last updated.
    cursor: usize,
    word: String,
    items: Vec<CompletionItem>,
    selected: usize,
    dismissed: Option<String>,
    /// Whether the next render should scroll the selected row into view.
    needs_scroll: bool,
    /// The selected row's rect from the last render.
    #[cfg(test)]
    selected_rect: Option<egui::Rect>,
}

#[derive(Clone)]
struct CompletionItem {
    insert: String,
    glyph: char,
    /// The name or alias that matched best, as shown in the popup.
    label: String,
    /// Character indices in `label` that matched the query.
    matched: Vec<usize>,
    /// The localized prototype name, when it differs from `label`.
    name: Option<String>,
    /// Character indices in `name` that matched the query.
    name_matched: Vec<usize>,
}

/// Consumes a key press only when its modifiers match exactly.
///
/// Unlike [`egui::InputState::consume_key`], this does not ignore extra
/// modifiers, so `Shift+Tab` can be told apart from plain `Tab`.
fn consume_unmodified_key(input: &mut egui::InputState, key: Key) -> bool {
    let mut consumed = false;
    input.events.retain(|event| {
        let matches = matches!(
            event,
            egui::Event::Key {
                key: event_key,
                modifiers,
                pressed: true,
                ..
            } if *event_key == key && !modifiers.any()
        );
        consumed |= matches;
        !matches
    });
    consumed
}

/// Word boundaries for completion. `:` is deliberately not one: it is part of
/// a plaintext prototype token.
fn is_completion_boundary(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '+' | '-' | '*' | '/' | '^' | '@' | '=' | '(' | ')' | '#'
        )
}

/// The word ending at `cursor`, as a char index and text.
fn completion_word(text: &str, cursor: usize) -> (usize, String) {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let mut start = cursor;
    while start > 0 && !is_completion_boundary(chars[start - 1]) {
        start -= 1;
    }

    // `-` separates words in arithmetic, but aliases and prototype IDs contain
    // it. When the word under the caret starts with a letter, rejoin the
    // dash-separated segments so `green-chip` is matched (and replaced) whole.
    if chars.get(start).is_some_and(|ch| ch.is_ascii_alphabetic()) {
        while start >= 2 && chars[start - 1] == '-' && !is_completion_boundary(chars[start - 2]) {
            start -= 1;
            while start > 0 && !is_completion_boundary(chars[start - 1]) {
                start -= 1;
            }
        }
    }

    (start, chars[start..cursor].iter().collect())
}

fn char_to_byte(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

fn insert_completion(text: &mut String, start: usize, len: usize, insert: &str) -> usize {
    let start_byte = char_to_byte(text, start);
    let end_byte = char_to_byte(text, start + len);
    text.replace_range(start_byte..end_byte, insert);
    start + insert.chars().count()
}

fn previous_word(before: &[char]) -> String {
    let mut word: Vec<char> = before
        .iter()
        .rev()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| !is_completion_boundary(**ch))
        .copied()
        .collect();
    word.reverse();
    word.into_iter().collect()
}

/// Common community names for prototypes.
const ALIASES: &[(&str, &[&str])] = &[
    ("electronic-circuit", &["green-chip", "green-circuit"]),
    ("advanced-circuit", &["red-chip", "red-circuit"]),
    ("processing-unit", &["blue-chip", "blue-circuit"]),
    ("transport-belt", &["yellow-belt"]),
    ("fast-transport-belt", &["red-belt"]),
    ("express-transport-belt", &["blue-belt"]),
    ("turbo-transport-belt", &["green-belt"]),
    ("electromagnetic-plant", &["em-plant"]),
    ("automation-science-pack", &["red-science-pack"]),
    ("logistic-science-pack", &["green-science-pack"]),
    (
        "military-science-pack",
        &[
            "gray-science-pack",
            "grey-science-pack",
            "black-science-pack",
        ],
    ),
    ("chemical-science-pack", &["blue-science-pack"]),
    ("production-science-pack", &["purple-science-pack"]),
    ("utility-science-pack", &["yellow-science-pack"]),
    ("space-science-pack", &["white-science-pack"]),
    (
        "metallurgic-science-pack",
        &["orange-science-pack", "vulcanus-science-pack"],
    ),
    (
        "agricultural-science-pack",
        &["lime-science-pack", "gleba-science-pack"],
    ),
    (
        "electromagnetic-science-pack",
        &["pink-science-pack", "fulgora-science-pack"],
    ),
    (
        "cryogenic-science-pack",
        &["cyan-science-pack", "aquilo-science-pack"],
    ),
];

fn aliases_for(id: &str) -> &'static [&'static str] {
    ALIASES
        .iter()
        .find_map(|(canonical, aliases)| (*canonical == id).then_some(*aliases))
        .unwrap_or(&[])
}

/// Weight of the reference-count popularity bonus added to fuzzy scores.
///
/// Fuzzy scores only compare match shape: `ipl` scores `ice-platform` a few
/// points above `iron-plate`, even though iron plates are referenced by far
/// more recipes. The logarithmic bonus lets a much more common symbol outrank
/// a near-equal match without letting popularity beat a clearly better match.
const COMPLETION_POPULARITY_WEIGHT: f64 = 2.0;

/// The ranking key of a completion: its fuzzy score plus a popularity bonus.
fn completion_rank(score: isize, references: u32) -> f64 {
    score as f64 + f64::from(references).ln_1p() * COMPLETION_POPULARITY_WEIGHT
}

/// The name of `entry` that best matches `needle`, with the matched char indices
/// and whether the localized display name was the best match.
fn best_name_match<'a>(
    entry: &'a hactorio_data::Entry,
    needle: &str,
) -> Option<(isize, &'a str, Vec<usize>, bool)> {
    let display_name = entry.display_name.as_deref();
    std::iter::once(entry.id.as_ref())
        .chain(display_name)
        .chain(aliases_for(&entry.id).iter().copied())
        .filter_map(|name| {
            let found = sublime_fuzzy::best_match(needle, name)?;
            let mut matched: Vec<usize> = found.matched_indices().copied().collect();
            matched.sort_unstable();
            Some((found.score(), name, matched, Some(name) == display_name))
        })
        .reduce(|best, candidate| {
            if candidate.0 > best.0 {
                candidate
            } else {
                best
            }
        })
}

fn completion_items(
    registry: &Registry,
    text: &str,
    word_start: usize,
    word: &str,
) -> Vec<CompletionItem> {
    let needle = word.strip_prefix(':').unwrap_or(word);
    if needle.is_empty() || needle.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        return Vec::new();
    }
    let before: Vec<char> = text.chars().take(word_start).collect();
    let context = if before.last() == Some(&'@') {
        CompletionContext::Machine
    } else if previous_word(&before) == "making" {
        CompletionContext::Recipe
    } else {
        CompletionContext::Normal
    };

    let mut items: Vec<(f64, CompletionItem)> = registry
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, entry)| match context {
            CompletionContext::Machine => entry.machine().is_some(),
            CompletionContext::Recipe => entry.recipe.is_some(),
            CompletionContext::Normal => true,
        })
        .filter_map(|(index, entry)| {
            let (score, matched_name, matched, display_matched) = best_name_match(entry, needle)?;
            let display_name = entry.display_name.as_deref();
            let (label, matched, name, name_matched) = if display_matched {
                // Show the canonical ID beside the prototype name that matched.
                (entry.id.as_ref(), Vec::new(), Some(matched_name), matched)
            } else if display_name.is_some_and(|name| name != matched_name) {
                (matched_name, matched, display_name, Vec::new())
            } else {
                (matched_name, matched, None, Vec::new())
            };
            let references = registry.reference_count(EntryId::from_index(index));
            Some((
                completion_rank(score, references),
                CompletionItem {
                    insert: entry.glyph.to_string(),
                    glyph: entry.glyph,
                    label: label.to_string(),
                    matched,
                    name: name.map(str::to_string),
                    name_matched,
                },
            ))
        })
        .collect();

    items.sort_by(|(a_rank, a), (b_rank, b)| {
        b_rank
            .total_cmp(a_rank)
            .then_with(|| a.label.len().cmp(&b.label.len()))
            .then_with(|| a.label.cmp(&b.label))
    });
    items.truncate(40);
    items.into_iter().map(|(_, item)| item).collect()
}

/// Spaces that separate the matched name from the prototype name.
const COMPLETION_NAME_GAP: usize = 4;

/// Number of padding spaces before the prototype name, for column alignment.
fn completion_padding(item: &CompletionItem, label_width: usize) -> usize {
    if item.name.is_some() {
        label_width.saturating_sub(item.label.chars().count()) + COMPLETION_NAME_GAP
    } else {
        0
    }
}

/// The full row text used for measuring.
fn completion_row_text(item: &CompletionItem, label_width: usize) -> String {
    let mut text = item.glyph.to_string();
    text.push(' ');
    text.push_str(&item.label);
    text.extend(std::iter::repeat_n(
        ' ',
        completion_padding(item, label_width),
    ));
    if let Some(name) = &item.name {
        text.push_str(name);
    }
    text
}

fn append_matched_text(
    job: &mut egui::text::LayoutJob,
    text: &str,
    matched: &[usize],
    normal: &egui::TextFormat,
    highlighted: &egui::TextFormat,
) {
    let mut matched = matched.iter().copied().peekable();
    let mut run = String::new();
    let mut run_matched = false;
    for (index, ch) in text.chars().enumerate() {
        let is_matched = matched.peek() == Some(&index);
        if is_matched {
            matched.next();
        }
        if is_matched != run_matched && !run.is_empty() {
            job.append(
                &run,
                0.0,
                if run_matched {
                    highlighted.clone()
                } else {
                    normal.clone()
                },
            );
            run.clear();
        }
        run_matched = is_matched;
        run.push(ch);
    }
    if !run.is_empty() {
        job.append(
            &run,
            0.0,
            if run_matched {
                highlighted.clone()
            } else {
                normal.clone()
            },
        );
    }
}

/// Builds one completion row, highlighting the fuzzy-matched characters.
fn completion_layout_job(
    item: &CompletionItem,
    label_width: usize,
    selected: bool,
    font_id: &egui::FontId,
    visuals: &egui::Visuals,
) -> egui::text::LayoutJob {
    // Selected rows use the standard text color; the dimmed color is hard to
    // read against the selection background.
    let normal = egui::TextFormat {
        font_id: font_id.clone(),
        color: if selected {
            visuals.strong_text_color()
        } else {
            visuals.text_color()
        },
        ..Default::default()
    };
    let strong = visuals.strong_text_color();
    let highlighted = egui::TextFormat {
        font_id: font_id.clone(),
        color: strong,
        underline: egui::Stroke::new(1.0, strong),
        ..Default::default()
    };

    let mut job = egui::text::LayoutJob::default();
    job.append(&item.glyph.to_string(), 0.0, normal.clone());
    job.append(" ", 0.0, normal.clone());
    append_matched_text(&mut job, &item.label, &item.matched, &normal, &highlighted);
    if let Some(name) = &item.name {
        job.append(
            &" ".repeat(completion_padding(item, label_width)),
            0.0,
            normal.clone(),
        );
        append_matched_text(&mut job, name, &item.name_matched, &normal, &highlighted);
    }
    job
}

fn truncate_text(
    painter: &egui::Painter,
    text: &str,
    font_id: &egui::FontId,
    max_width: f32,
) -> String {
    let galley = painter.layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE);
    if galley.size().x <= max_width {
        return text.to_string();
    }
    let ellipsis = painter
        .layout_no_wrap("…".to_string(), font_id.clone(), Color32::WHITE)
        .size()
        .x;
    let limit = (max_width - ellipsis).max(0.0);
    let mut keep = 0;
    if let Some(row) = galley.rows.first() {
        for glyph in &row.row.glyphs {
            if glyph.pos.x + glyph.advance_width <= limit {
                keep += 1;
            } else {
                break;
            }
        }
    }
    let mut shortened: String = text.chars().take(keep).collect();
    shortened.push('…');
    shortened
}

/// Draws a monospace value and returns the rect of any hovered icon glyph in it.
fn icon_label(ui: &mut Ui, text: &str, registry: &Registry) -> Option<egui::Rect> {
    let font_id = TextStyle::Monospace.resolve(ui.style());
    let color = ui.visuals().text_color();
    let galley = ui
        .painter()
        .layout(text.to_string(), font_id, color, ui.available_width());
    let (rect, _response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley.clone(), color);

    let pointer = ui.ctx().pointer_hover_pos()?;
    hovered_icon(registry, &galley, rect.min, pointer).map(|(_, rect)| rect)
}

fn detail_ui(
    ui: &mut Ui,
    statement: &StatementOutput,
    settings: &Settings,
    registry: &Registry,
) -> Option<egui::Rect> {
    let mut hovered = None;
    match &statement.output {
        LineOutput::Nothing => {}
        LineOutput::Error(error) => {
            ui.label(RichText::new(&error.message).color(Color32::from_rgb(220, 90, 90)));
        }
        LineOutput::Value(value) => {
            let formatted = format_output(value, settings, registry);
            hovered = icon_label(ui, &formatted.headline, registry);

            let Output::Recipe(recipe) = value else {
                return hovered;
            };
            let rate = matches!(recipe.quantity, RecipeQuantity::Machines(_));

            let mut units: Vec<(BaseUnit, f64)> = recipe
                .net
                .iter()
                .filter(|(unit, _)| **unit != BaseUnit::Craft)
                .map(|(unit, amount)| (*unit, *amount))
                .collect();
            units.sort_by_key(|(unit, _)| unit_label(registry, *unit));

            let mut produces = Vec::new();
            let mut consumes = Vec::new();
            for (unit, amount) in units {
                let text = format_amount(amount.abs(), unit, rate, settings, registry);
                if amount > 0.0 {
                    produces.push(text);
                } else {
                    consumes.push(text);
                }
            }

            egui::Grid::new("detail_items")
                .num_columns(2)
                .min_col_width(90.0)
                .spacing([18.0, 2.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("produces").weak());
                    ui.label(RichText::new("consumes").weak());
                    ui.end_row();
                    for row in 0..produces.len().max(consumes.len()) {
                        let produce = produces.get(row).map(String::as_str).unwrap_or("");
                        let consume = consumes.get(row).map(String::as_str).unwrap_or("");
                        hovered = hovered.or(icon_label(ui, produce, registry));
                        hovered = hovered.or(icon_label(ui, consume, registry));
                        ui.end_row();
                    }
                });

            if let Some(craft) = recipe.net.get(&BaseUnit::Craft).copied() {
                let value = if rate {
                    Value::new(craft, [(BaseUnit::Craft, 1), (BaseUnit::Second, -1)])
                } else {
                    Value::crafts(craft)
                };
                hovered = hovered.or(icon_label(
                    ui,
                    &format_value(&value, settings, registry),
                    registry,
                ));
            }

            let glyph = registry.entry_by_index(recipe.machine).glyph;
            hovered = hovered.or(icon_label(
                ui,
                &format!(
                    "{}/{glyph}",
                    format_power(recipe.per_machine_power_w, settings)
                ),
                registry,
            ));
            let count = recipe.quantity.magnitude();
            if rate && count != 1.0 {
                hovered = hovered.or(icon_label(
                    ui,
                    &format!(
                        "{} total",
                        format_power(recipe.per_machine_power_w * count, settings)
                    ),
                    registry,
                ));
            }
            if !rate && let Some(energy) = recipe.total_energy_j {
                hovered = hovered.or(icon_label(
                    ui,
                    &format!("{} total energy", format_energy(energy, settings)),
                    registry,
                ));
            }
        }
    }
    hovered
}

fn unit_label(registry: &Registry, unit: BaseUnit) -> String {
    match unit {
        BaseUnit::Item(id) | BaseUnit::Fluid(id) => registry.entry_by_index(id).id.to_string(),
        BaseUnit::Craft => "craft".to_string(),
        BaseUnit::Second => "second".to_string(),
        BaseUnit::Joule => "joule".to_string(),
    }
}

fn output_text(
    statement: &StatementOutput,
    settings: &Settings,
    registry: &Registry,
) -> Option<String> {
    match &statement.output {
        LineOutput::Nothing => None,
        LineOutput::Error(error) => Some(error.message.clone()),
        LineOutput::Value(value) => {
            let formatted = format_output(value, settings, registry);
            let mut text = formatted.headline;
            for row in formatted.rows {
                text.push_str(" · ");
                text.push_str(&row);
            }
            Some(text)
        }
    }
}

#[cfg(test)]
mod tests;
