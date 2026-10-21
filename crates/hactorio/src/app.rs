//! The document window: tabs, line editor with outputs, settings, breakdown.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{
    Align, Color32, ComboBox, Context, Key, Layout, RichText, ScrollArea, TextEdit, TextStyle,
    Ui,
};
use hactorio_core::{
    ItemRateDisplay, LineOutput, Settings, StatementOutput, TimeUnit, breakdown, format_output,
    from_plaintext, line_hash, to_plaintext,
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
    focus_request: Option<usize>,
    install: Option<GameInstall>,
    load: LoadState,
    status: String,
    dirty: bool,
    last_save: Instant,
    app_dir: PathBuf,
    initial_size: Option<[f32; 2]>,
    sized: bool,
    window: Option<Window>,
}

enum LoadState {
    Idle,
    Running(Receiver<Result<(Registry, GameInstall), String>>),
}

impl HactorioApp {
    pub fn new(cc: &eframe::CreationContext<'_>, document: Document, app_dir: PathBuf) -> Self {
        let mut tabs: Vec<Tab> = if document.tabs.is_empty() {
            vec![Tab::from_text("# Main\n")]
        } else {
            document.tabs.iter().map(|text| Tab::from_text(text)).collect()
        };
        let active = document.active_tab.min(tabs.len() - 1);
        for (tab, cursor) in tabs.iter_mut().zip(document.cursors.iter()) {
            tab.cursor_line = cursor.line.min(tab.lines.len().saturating_sub(1));
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
            icons::install(
                &cc.egui_ctx,
                Arc::clone(registry),
                &install.icon_source(),
            );
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
            focus_request: None,
            install,
            load: LoadState::Idle,
            status,
            dirty: false,
            last_save: Instant::now(),
            app_dir,
            initial_size: document.window.map(|window| [window.width, window.height]),
            sized: false,
            window: document.window,
        }
    }

    fn current_text(&self) -> String {
        self.tabs
            .get(self.active)
            .map(Tab::text)
            .unwrap_or_default()
    }

    fn poll_evaluator(&mut self) {
        while let Some(response) = self.evaluator.poll() {
            if response.generation == self.pending {
                self.outputs = response.outputs;
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
                let text = tab.text();
                let plain = to_plaintext(&text, old);
                tab.set_text(&from_plaintext(&plain, &registry));
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
        let (Some(text), Some(output)) = (tab.lines.get(line), self.outputs.get(line)) else {
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

    fn snapshot(&self) -> Document {
        Document {
            version: DOCUMENT_VERSION,
            tabs: self.tabs.iter().map(Tab::text).collect(),
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

impl eframe::App for HactorioApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
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

        egui::Panel::top("tabs").show(ui, |ui| {
            self.tabs_bar(ui);
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).small().weak());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let lines = self.tabs[self.active].lines.len();
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
            let focus_request = self.focus_request.take();
            let result = editor_ui(
                ui,
                &mut self.tabs[active],
                active,
                &self.outputs,
                &self.settings,
                self.registry.as_deref(),
                focus_request,
            );
            if result.changed {
                self.dirty = true;
                self.last_request = None;
            }
            if let Some(line) = result.focused {
                self.focus_line = Some(line);
            }
            if let Some(line) = result.focus {
                self.focus_line = Some(line);
                self.focus_request = Some(line);
            }
        });

        if self.dirty && self.last_save.elapsed() >= SAVE_INTERVAL {
            self.save();
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save();
    }
}

struct EditorResult {
    changed: bool,
    /// A line the user is editing now.
    focused: Option<usize>,
    /// A line to focus next frame, after inserting or deleting.
    focus: Option<usize>,
}

/// Renders one tab's lines with their outputs, and handles line insertion.
fn editor_ui(
    ui: &mut Ui,
    tab: &mut Tab,
    active: usize,
    outputs: &[StatementOutput],
    settings: &Settings,
    registry: Option<&Registry>,
    focus_request: Option<usize>,
) -> EditorResult {
    let mut result = EditorResult {
        changed: false,
        focused: None,
        focus: None,
    };
    let mut enter_at: Option<(usize, usize)> = None;
    let mut remove_line: Option<usize> = None;

    let scroll = ScrollArea::vertical()
        .id_salt(("editor", active))
        .vertical_scroll_offset(tab.scroll)
        .show(ui, |ui| {
            for index in 0..tab.lines.len() {
                let line_id = ui.make_persistent_id(("line", active, index));
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [28.0, 18.0],
                        egui::Label::new(
                            RichText::new(format!("{:>3}", index + 1))
                                .weak()
                                .monospace(),
                        ),
                    );

                    let available = ui.available_width();
                    let editor_width = (available * 0.55).max(120.0);
                    let response = ui.add_sized(
                        [editor_width, 18.0],
                        TextEdit::singleline(&mut tab.lines[index])
                            .id(line_id)
                            .font(TextStyle::Monospace),
                    );
                    result.changed |= response.changed();
                    if response.has_focus() {
                        result.focused = Some(index);
                        tab.cursor_line = index;
                    }
                    if focus_request == Some(index) {
                        ui.memory_mut(|memory| memory.request_focus(line_id));
                    }

                    if response.has_focus() {
                        if ui.input(|input| input.key_pressed(Key::Enter)) {
                            let chars = egui::text_edit::TextEditState::load(ui.ctx(), line_id)
                                .and_then(|state| state.cursor.char_range())
                                .map(|range| range.primary.index.0)
                                .unwrap_or(usize::MAX);
                            enter_at = Some((index, chars));
                        }
                        if tab.lines[index].is_empty()
                            && tab.lines.len() > 1
                            && ui.input(|input| input.key_pressed(Key::Backspace))
                        {
                            remove_line = Some(index);
                        }
                    }

                    ui.add_space(6.0);
                    if let (Some(output), Some(registry)) = (outputs.get(index), registry)
                        && output.source_hash == line_hash(&tab.lines[index])
                    {
                        output_ui(ui, output, settings, registry);
                    }
                });
            }
        });
    tab.scroll = scroll.state.clamped_offset().y;

    if let Some((index, chars)) = enter_at {
        let byte = char_to_byte(&tab.lines[index], chars);
        let tail = tab.lines[index].split_off(byte);
        tab.lines.insert(index + 1, tail);
        tab.cursor_line = index + 1;
        result.focus = Some(index + 1);
    }

    if let Some(index) = remove_line {
        tab.lines.remove(index);
        let previous = index.saturating_sub(1);
        tab.cursor_line = previous;
        result.focus = Some(previous);
    }

    result
}

fn char_to_byte(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

fn output_ui(
    ui: &mut Ui,
    output: &StatementOutput,
    settings: &Settings,
    registry: &Registry,
) {
    match &output.output {
        LineOutput::Nothing => {}
        LineOutput::Error(error) => {
            ui.label(
                RichText::new(&error.message)
                    .small()
                    .color(Color32::from_rgb(220, 90, 90)),
            );
        }
        LineOutput::Value(value) => {
            let formatted = format_output(value, settings, registry);
            let mut text = formatted.headline;
            for row in formatted.rows {
                text.push('\n');
                text.push_str(&row);
            }
            ui.label(
                RichText::new(text)
                    .monospace()
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        }
    }
}
