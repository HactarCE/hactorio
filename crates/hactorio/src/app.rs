//! The document window: tabs, line editor with outputs, settings, breakdown.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{
    Align, Color32, ComboBox, Context, Key, Layout, Modifiers, RichText, ScrollArea, TextEdit, TextStyle,
    Ui,
};
use hactorio_core::{
    BaseUnit, ItemRateDisplay, LineOutput, Output, RecipeQuantity, Settings, StatementOutput,
    TimeUnit, Value, breakdown, format_amount, format_energy, format_output, format_power,
    format_value, from_plaintext, line_hash, to_plaintext,
};
use hactorio_data::{GameInstall, Registry, locate};

use crate::document::{Cursor, DOCUMENT_VERSION, Document, Tab, Window};
use crate::evaluator::Evaluator;
use crate::icons;

const SAVE_INTERVAL: Duration = Duration::from_secs(5);

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

pub struct HactorioApp {
    tabs: Vec<Tab>,
    active: usize,
    settings: Settings,
    evaluator: Evaluator,
    registry: Option<Arc<Registry>>,
    outputs: Vec<StatementOutput>,
    pending: u64,
    last_request: Option<(String, Settings)>,
    focus_line: Option<usize>,
    install: Option<GameInstall>,
    load: LoadState,
    status: String,
    dirty: bool,
    last_save: Instant,
    app_dir: PathBuf,
    initial_size: Option<[f32; 2]>,
    sized: bool,
    window: Option<Window>,
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
            document.tabs.iter().map(|text| Tab::from_text(text)).collect()
        };
        let active = document.active_tab.min(tabs.len() - 1);
        for (tab, cursor) in tabs.iter_mut().zip(document.cursors.iter()) {
            tab.cursor_line = cursor.line.min(tab.text.split('\n').count().saturating_sub(1));
            tab.scroll = cursor.scroll;
        }

        let cache_path = app_dir.join("cache.json");
        let (registry, status) = match Registry::load(&cache_path) {
            Ok(registry) => {
                let status = format!("{} prototypes", registry.entries().len());
                (Some(Arc::new(registry)), status)
            }
            Err(error) if cache_path.exists() => (None, format!("Game data failed: {error}")),
            Err(_) => (None, "No game data yet; use \"Load game data\".".to_string()),
        };
        let install = locate();
        if let (Some(registry), Some(install)) = (&registry, &install) {
            icons::install(ctx, Arc::clone(registry), &install.icon_source());
        }

        HactorioApp {
            tabs,
            active,
            settings: document.settings,
            evaluator: Evaluator::new(),
            registry,
            outputs: Vec::new(),
            pending: 0,
            last_request: None,
            focus_line: None,
            install,
            load: LoadState::Idle,
            status,
            dirty: false,
            last_save: Instant::now(),
            app_dir,
            initial_size: document.window.map(|window| [window.width, window.height]),
            sized: false,
            window: document.window,
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
            self.focus_line = None;
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

    fn settings_ui(&mut self, ui: &mut Ui) {
        let before = self.settings.clone();
        ui.heading("Settings");

        ComboBox::from_label("Time unit")
            .selected_text(self.settings.time_unit.name())
            .show_ui(ui, |ui| {
                for unit in [TimeUnit::Second, TimeUnit::Minute, TimeUnit::Hour] {
                    ui.selectable_value(&mut self.settings.time_unit, unit, unit.name());
                }
            });

        let display_label = match self.settings.item_rate_display.belt() {
            None => "per second",
            Some(_) => self.settings.item_rate_display.token().unwrap_or(""),
        };
        ComboBox::from_label("Item rates")
            .selected_text(display_label)
            .show_ui(ui, |ui| {
                for display in [
                    ItemRateDisplay::PerSecond,
                    ItemRateDisplay::Yb,
                    ItemRateDisplay::Rb,
                    ItemRateDisplay::Bb,
                    ItemRateDisplay::Gb,
                    ItemRateDisplay::Syb,
                    ItemRateDisplay::Srb,
                    ItemRateDisplay::Sbb,
                    ItemRateDisplay::Sgb,
                ] {
                    let label = display.token().unwrap_or("per second");
                    ui.selectable_value(&mut self.settings.item_rate_display, display, label);
                }
            });

        ui.add(egui::Slider::new(&mut self.settings.max_decimals, 0..=6).text("Decimals"));
        ui.checkbox(&mut self.settings.metric_prefixes, "Metric prefixes");

        ComboBox::from_label("Belt stacking")
            .selected_text(self.settings.belt_stacking.to_string())
            .show_ui(ui, |ui| {
                for level in 1..=4u8 {
                    ui.selectable_value(
                        &mut self.settings.belt_stacking,
                        level,
                        level.to_string(),
                    );
                }
            });

        if self.settings != before {
            self.dirty = true;
            self.last_request = None;
        }
    }

    fn breakdown_ui(&self, ui: &mut Ui) {
        let (Some(registry), Some(tab)) = (&self.registry, self.tabs.get(self.active)) else {
            return;
        };
        let Some(line) = self.focus_line else {
            ui.weak("Focus a line to see how it was computed.");
            return;
        };
        let (Some(text), Some(output)) = (
            tab.text.split('\n').nth(line),
            self.outputs.get(line),
        ) else {
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
        let pos = egui::pos2(
            anchor.x.min(screen.right() - width).max(screen.left() + 8.0),
            anchor.y.min(screen.bottom() - 160.0).max(screen.top() + 8.0),
        );
        egui::Area::new(egui::Id::unique("line_detail"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width((width - 20.0).max(240.0));
                    ui.set_max_width(width);
                    detail_ui(ui, statement, &self.settings, registry);
                });
            });
    }

    fn snapshot(&self) -> Document {
        Document {
            version: DOCUMENT_VERSION,
            tabs: self.tabs.iter().map(|tab| tab.text.clone()).collect(),
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

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).small().weak());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let lines = self.tabs[self.active].text.split('\n').count();
                    ui.label(RichText::new(format!("{lines} lines")).small().weak());
                });
            });
        });

        egui::Panel::right("side")
            .default_size(380.0)
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("side_scroll")
                    .show(ui, |ui| {
                        self.settings_ui(ui);
                        ui.separator();
                        self.breakdown_ui(ui);
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
            };
            let result = editor_ui(ui, &mut self.tabs[active], &mut state);
            if result.changed {
                self.dirty = true;
                self.last_request = None;
            }
            if let Some(line) = result.focused {
                self.focus_line = Some(line);
            }
            if let Some((line, anchor, width)) = result.detail
                && self.completion.items.is_empty()
            {
                self.detail_panel(&ctx, anchor, line, width);
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
}

struct EditorResult {
    changed: bool,
    /// The line the caret is on, while the editor has focus.
    focused: Option<usize>,
    /// Where to place the full-results panel for the focused line, and its width.
    detail: Option<(usize, egui::Pos2, f32)>,
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

    let mut result = EditorResult {
        changed: false,
        focused: None,
        detail: None,
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
    let central_width = ui.available_width();
    let gutter_width = (central_width * 0.45).max(180.0);
    let editor_width = (central_width - number_width - gutter_width - 28.0).max(200.0);
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
                        accept = input.consume_key(Modifiers::NONE, Key::Enter)
                            || input.consume_key(Modifiers::NONE, Key::Tab);
                        dismiss = input.consume_key(Modifiers::NONE, Key::Escape);
                        move_selection = down - up;
                    });
                }

                let text_id = ui.make_persistent_id(("text", active));
                let hscroll = ScrollArea::horizontal()
                    .id_salt(("text_scroll", active))
                    .max_width(editor_width)
                    .show(ui, |ui| {
                        TextEdit::multiline(&mut tab.text)
                            .code_editor()
                            .desired_width(editor_content_width)
                            .desired_rows(line_count.max(6))
                            .margin(egui::Margin::symmetric(4, 4))
                            .layouter(&mut layouter)
                            .id(text_id)
                            .show(ui)
                    });
                let text_edit = hscroll.inner;
                result.changed |= text_edit.response.changed();

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
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
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
                if !text_edit.response.has_focus() {
                    completion.items.clear();
                }

                if let Some(registry) = registry {
                    let text_changed = completion.text_hash != line_hash(&tab.text);
                    if text_changed || word != completion.word {
                        completion.text_hash = line_hash(&tab.text);
                        completion.word = word.clone();
                        completion.selected = 0;
                        completion.items = if word.is_empty() {
                            Vec::new()
                        } else {
                            completion_items(registry, &tab.text, word_start, &word)
                        };
                        if completion.dismissed.as_deref() != Some(word.as_str()) {
                            completion.dismissed = None;
                        }
                    }
                }

                if popup_open && !completion.items.is_empty() {
                    if dismiss {
                        completion.dismissed = Some(completion.word.clone());
                        completion.items.clear();
                    } else {
                        if move_selection != 0 {
                            let len = completion.items.len() as i32;
                            completion.selected =
                                (completion.selected as i32 + move_selection).rem_euclid(len)
                                    as usize;
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
                        } else {
                            let caret = text_edit
                                .galley
                                .pos_from_cursor(egui::text::CCursor::new(cursor_char));
                            let anchor =
                                text_edit.galley_pos + caret.left_bottom().to_vec2() + egui::vec2(0.0, 2.0);
                            let screen = ui.ctx().viewport_rect();
                            let pos = egui::pos2(
                                anchor.x.min(screen.right() - 340.0).max(screen.left() + 8.0),
                                anchor.y.min(screen.bottom() - 100.0).max(screen.top() + 8.0),
                            );
                            let mut clicked = None;
                            egui::Area::new(egui::Id::unique(("completion", active)))
                                .order(egui::Order::Foreground)
                                .fixed_pos(pos)
                                .show(ui.ctx(), |ui| {
                                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                                        ui.set_min_width(300.0);
                                        ui.set_max_width(420.0);
                                        egui::ScrollArea::vertical()
                                            .max_height(240.0)
                                            .show(ui, |ui| {
                                                for (index, item) in
                                                    completion.items.iter().enumerate()
                                                {
                                                    let mut label = format!(
                                                        "{} {}",
                                                        item.glyph, item.id
                                                    );
                                                    if let Some(description) =
                                                        &item.description
                                                    {
                                                        label.push_str("   ");
                                                        label.push_str(description);
                                                    }
                                                    if ui
                                                        .selectable_label(
                                                            index == completion.selected,
                                                            RichText::new(label).monospace(),
                                                        )
                                                        .clicked()
                                                    {
                                                        clicked = Some(index);
                                                    }
                                                }
                                            });
                                    });
                                });
                            ui.memory_mut(|memory| memory.request_focus(text_id));
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
                                        egui::text::CCursorRange::one(
                                            egui::text::CCursor::new(inserted),
                                        ),
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
                let gutter_x = viewport.right() + 12.0;
                let gutter_room = (ui.available_width() - 16.0).max(120.0);
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
                    painter.text(
                        egui::pos2(gutter_x, y),
                        egui::Align2::LEFT_TOP,
                        text,
                        font_id.clone(),
                        color,
                    );
                }

                ui.add_space(8.0);
                ui.allocate_space(egui::vec2(gutter_width, 0.0));
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
    text_hash: u64,
    word: String,
    items: Vec<CompletionItem>,
    selected: usize,
    dismissed: Option<String>,
}

#[derive(Clone)]
struct CompletionItem {
    insert: String,
    glyph: char,
    id: String,
    description: Option<String>,
}

/// Word boundaries for completion. `:` is deliberately not one: it is part of
/// a plaintext prototype token.
fn is_completion_boundary(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '+' | '-' | '*' | '/' | '^' | '@' | '=' | '(' | ')' | '#')
}

/// The word ending at `cursor`, as a char index and text.
fn completion_word(text: &str, cursor: usize) -> (usize, String) {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let mut start = cursor;
    while start > 0 && !is_completion_boundary(chars[start - 1]) {
        start -= 1;
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

fn is_subsequence(haystack: &str, needle: &str) -> bool {
    let mut chars = haystack.chars();
    needle.chars().all(|wanted| chars.any(|ch| ch == wanted))
}

fn completion_items(
    registry: &Registry,
    text: &str,
    word_start: usize,
    word: &str,
) -> Vec<CompletionItem> {
    if word.is_empty() {
        return Vec::new();
    }
    let needle = word.strip_prefix(':').unwrap_or(word).to_lowercase();
    let before: Vec<char> = text.chars().take(word_start).collect();
    let context = if before.last() == Some(&'@') {
        CompletionContext::Machine
    } else if previous_word(&before) == "making" {
        CompletionContext::Recipe
    } else {
        CompletionContext::Normal
    };

    let mut entries: Vec<&hactorio_data::Entry> = registry
        .entries()
        .iter()
        .filter(|entry| match context {
            CompletionContext::Machine => entry.machine().is_some(),
            CompletionContext::Recipe => entry.recipe.is_some(),
            CompletionContext::Normal => true,
        })
        .filter(|entry| needle.is_empty() || is_subsequence(&entry.id.to_lowercase(), &needle))
        .collect();

    entries.sort_by(|a, b| {
        let a_prefix = !needle.is_empty() && a.id.to_lowercase().starts_with(&needle);
        let b_prefix = !needle.is_empty() && b.id.to_lowercase().starts_with(&needle);
        b_prefix
            .cmp(&a_prefix)
            .then_with(|| a.id.len().cmp(&b.id.len()))
            .then_with(|| a.id.cmp(&b.id))
    });
    entries.truncate(40);

    entries
        .into_iter()
        .map(|entry| CompletionItem {
            insert: entry.glyph.to_string(),
            glyph: entry.glyph,
            id: entry.id.to_string(),
            description: entry.display_name.as_ref().map(|name| name.to_string()),
        })
        .collect()
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

fn detail_ui(
    ui: &mut Ui,
    statement: &StatementOutput,
    settings: &Settings,
    registry: &Registry,
) {
    match &statement.output {
        LineOutput::Nothing => {}
        LineOutput::Error(error) => {
            ui.label(
                RichText::new(&error.message).color(Color32::from_rgb(220, 90, 90)),
            );
        }
        LineOutput::Value(value) => {
            let formatted = format_output(value, settings, registry);
            ui.label(RichText::new(formatted.headline).monospace());

            let Output::Recipe(recipe) = value else {
                return;
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
                        ui.label(
                            RichText::new(
                                produces.get(row).map(String::as_str).unwrap_or(""),
                            )
                            .monospace(),
                        );
                        ui.label(
                            RichText::new(
                                consumes.get(row).map(String::as_str).unwrap_or(""),
                            )
                            .monospace(),
                        );
                        ui.end_row();
                    }
                });

            if let Some(craft) = recipe.net.get(&BaseUnit::Craft).copied() {
                let value = if rate {
                    Value::new(craft, [(BaseUnit::Craft, 1), (BaseUnit::Second, -1)])
                } else {
                    Value::crafts(craft)
                };
                ui.label(RichText::new(format_value(&value, settings, registry)).monospace());
            }

            let glyph = registry.entry_by_index(recipe.machine).glyph;
            ui.label(
                RichText::new(format!(
                    "{}/{glyph}",
                    format_power(recipe.per_machine_power_w, settings)
                ))
                .monospace(),
            );
            let count = recipe.quantity.magnitude();
            if rate && count != 1.0 {
                ui.label(
                    RichText::new(format!(
                        "{} total",
                        format_power(recipe.per_machine_power_w * count, settings)
                    ))
                    .monospace(),
                );
            }
            if !rate && let Some(energy) = recipe.total_energy_j {
                ui.label(
                    RichText::new(format!(
                        "{} total energy",
                        format_energy(energy, settings)
                    ))
                    .monospace(),
                );
            }
        }
    }
}

fn unit_label(registry: &Registry, unit: BaseUnit) -> String {
    match unit {
        BaseUnit::Item(id) | BaseUnit::Fluid(id) => {
            registry.entry_by_index(id).id.to_string()
        }
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
mod tests {
    use super::*;
    use crate::document::{Cursor, Document};
    use egui_kittest::Harness;

    fn sample_document() -> Document {
        let text = "# Main\n\
            rate = 3 :iron-plate: / second\n\
            100 :electronic-circuit: / second in @:assembling-machine-3: making :electronic-circuit:\n\
            @:electric-furnace: 2 :productivity-module-3: making :iron-plate:\n\
            100 :iron-plate: in @:electric-furnace:\n\
            2 sgb :iron-plate:\n\
            3 m + 4\n";
        Document {
            tabs: vec![text.to_string()],
            cursors: vec![Cursor {
                line: 3,
                column: 0,
                scroll: 0.0,
            }],
            ..Document::default()
        }
    }

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

    const FIXTURE: &str = r#"{
      "item": {
        "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100, "icon": "a.png"},
        "copper-cable": {"type": "item", "name": "copper-cable", "stack_size": 200, "icon": "b.png"},
        "assembling-machine-3": {"type": "item", "name": "assembling-machine-3", "stack_size": 50, "icon": "c.png"}
      },
      "fluid": {"water": {"type": "fluid", "name": "water", "icon": "d.png"}},
      "module": {"speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}}},
      "recipe": {
        "iron-plate": {"type": "recipe", "name": "iron-plate", "categories": ["smelting"], "results": [{"type": "item", "name": "iron-plate", "amount": 1}]},
        "electronic-circuit": {"type": "recipe", "name": "electronic-circuit", "categories": ["crafting"], "results": [{"type": "item", "name": "iron-plate", "amount": 1}]}
      },
      "assembling-machine": {
        "assembling-machine-3": {"type": "assembling-machine", "name": "assembling-machine-3", "crafting_categories": ["crafting"], "crafting_speed": 1.25, "module_slots": 4, "energy_usage": "375kW"}
      }
    }"#;

    fn fixture_registry() -> Registry {
        hactorio_data::postprocess(FIXTURE, &hactorio_data::Locale::empty()).unwrap()
    }

    fn suggested_ids(registry: &Registry, text: &str) -> Vec<String> {
        let cursor = text.chars().count();
        let (start, word) = completion_word(text, cursor);
        completion_items(registry, text, start, &word)
            .into_iter()
            .map(|item| item.insert)
            .collect()
    }

    #[test]
    fn completion_matches_subsequences_case_insensitively() {
        let registry = fixture_registry();
        let inserts = suggested_ids(&registry, ":IPL");
        let iron = registry.entry("iron-plate").unwrap().glyph.to_string();
        assert!(inserts.contains(&iron), "{inserts:?}");
    }

    #[test]
    fn completion_after_at_only_offers_machines() {
        let registry = fixture_registry();
        let machine = registry.entry("assembling-machine-3").unwrap().glyph.to_string();
        let plate = registry.entry("iron-plate").unwrap().glyph.to_string();

        let inserts = suggested_ids(&registry, "@asm");
        assert!(inserts.contains(&machine), "{inserts:?}");

        let inserts = suggested_ids(&registry, "@ipl");
        assert!(!inserts.contains(&plate), "{inserts:?}");
    }

    #[test]
    fn completion_after_making_only_offers_recipes() {
        let registry = fixture_registry();
        let circuit = registry.entry("electronic-circuit").unwrap().glyph.to_string();
        let water = registry.entry("water").unwrap().glyph.to_string();

        let inserts = suggested_ids(&registry, "making :circ");
        assert!(inserts.contains(&circuit), "{inserts:?}");

        let inserts = suggested_ids(&registry, "making :water");
        assert!(!inserts.contains(&water), "{inserts:?}");
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
}
