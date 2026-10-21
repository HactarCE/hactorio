//! The document window: tabs, line editor with outputs, settings, breakdown.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{
    Align, Color32, ComboBox, Context, Layout, RichText, ScrollArea, TextEdit, TextStyle,
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
            .map(|tab| tab.text.clone())
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
    fn detail_panel(&self, ctx: &Context, anchor: egui::Pos2, line: usize) {
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
        let pos = egui::pos2(
            anchor.x.min(screen.right() - 420.0).max(screen.left() + 8.0),
            anchor.y.min(screen.bottom() - 160.0).max(screen.top() + 8.0),
        );
        egui::Area::new(egui::Id::unique("line_detail"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(420.0);
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
            let result = editor_ui(
                ui,
                &mut self.tabs[active],
                active,
                &self.outputs,
                &self.settings,
                self.registry.as_deref(),
            );
            if result.changed {
                self.dirty = true;
                self.last_request = None;
            }
            if let Some(line) = result.focused {
                self.focus_line = Some(line);
            }
            if let Some((line, anchor)) = result.detail {
                self.detail_panel(&ctx, anchor, line);
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
    /// The line the caret is on, while the editor has focus.
    focused: Option<usize>,
    /// Where to place the full-results panel for the focused line.
    detail: Option<(usize, egui::Pos2)>,
}

/// Renders the tab as one multiline editor, with line numbers and outputs
/// painted at the galley's row positions so they stay aligned.
fn editor_ui(
    ui: &mut Ui,
    tab: &mut Tab,
    active: usize,
    outputs: &[StatementOutput],
    settings: &Settings,
    registry: Option<&Registry>,
) -> EditorResult {
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
    let gutter_width = (ui.available_width() * 0.45).max(180.0);
    let editor_width = (ui.available_width() - number_width - gutter_width - 24.0)
        .max(longest as f32 * char_width + 24.0)
        .max(200.0);

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

    let scroll = ScrollArea::both()
        .id_salt(("editor", active))
        .vertical_scroll_offset(tab.scroll)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add_space(number_width);
                let text_edit = TextEdit::multiline(&mut tab.text)
                    .code_editor()
                    .desired_width(editor_width)
                    .desired_rows(line_count.max(6))
                    .margin(egui::Margin::symmetric(4, 4))
                    .layouter(&mut layouter)
                    .id_salt(("text", active))
                    .show(ui);
                result.changed |= text_edit.response.changed();

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

                let frame = text_edit.response.rect;
                let galley = &text_edit.galley;
                let origin = text_edit.galley_pos;
                let weak = ui.visuals().weak_text_color();
                let error_color = Color32::from_rgb(220, 90, 90);
                let painter = ui.painter();

                for (index, line) in tab.text.split('\n').enumerate() {
                    let Some(row) = galley.rows.get(index) else {
                        break;
                    };
                    let y = origin.y + row.pos.y;
                    painter.text(
                        egui::pos2(origin.x - 10.0, y),
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
                        result.detail = Some((
                            index,
                            egui::pos2(frame.max.x + 4.0, y),
                        ));
                        continue;
                    }
                    let color = if matches!(statement.output, LineOutput::Error(_)) {
                        error_color
                    } else {
                        weak
                    };
                    let text = truncate_text(painter, &text, &font_id, gutter_width - 8.0);
                    painter.text(
                        egui::pos2(frame.max.x + 12.0, y),
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
                    "{} /{glyph}",
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
