//! The document window: tabs, line editor with results, settings, and tokens.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Align, Context, RichText, TextStyle, Ui};
use hactorio_core::belt::BeltTier;
use hactorio_core::new_eval::Evaluation;
use hactorio_core::{IdlePower, Settings, TimeUnit, from_plaintext, to_plaintext};
use hactorio_data::{GameInstall, Registry, locate};

use crate::document::{
    Cursor, DOCUMENT_VERSION, Document, MAX_FONT_SIZE, MAX_ZOOM, MIN_FONT_SIZE, MIN_ZOOM, Tab,
    Window,
};
use crate::evaluator::Evaluator;
use crate::fonts;
use crate::glyphs;
use crate::{APP_TITLE, clipboard};

mod completion;
mod editor;
mod highlight;
mod results;
mod theme;

use completion::CompletionState;
use editor::{EditorState, editor_ui};

const SAVE_INTERVAL: Duration = Duration::from_secs(5);

/// Font size of glyphs, such as belt tiers.
const GLYPH_SIZE: f32 = 18.0;

/// Sets the monospace text size globally, so the editor, results column, and popups agree.
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

/// Height of a button whose text is a glyph.
fn glyph_button_height(ui: &Ui) -> f32 {
    let font = egui::FontId::proportional(GLYPH_SIZE);
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

#[derive(Clone, Copy)]
enum DisplaySetting {
    Theme,
    Font,
    TimeUnit,
    ItemRates,
    Decimals,
    MetricPrefixes,
}

const DISPLAY_SETTINGS: &[DisplaySetting] = &[
    DisplaySetting::Theme,
    DisplaySetting::Font,
    DisplaySetting::TimeUnit,
    DisplaySetting::ItemRates,
    DisplaySetting::Decimals,
    DisplaySetting::MetricPrefixes,
];

#[derive(Clone, Copy)]
enum ResearchSetting {
    BeltStacking,
    InserterCapacity,
    Productivity,
}

const RESEARCH_SETTINGS: &[ResearchSetting] = &[
    ResearchSetting::BeltStacking,
    ResearchSetting::InserterCapacity,
    ResearchSetting::Productivity,
];

#[derive(Clone, Copy)]
enum CalculationSetting {
    IdlePower,
}

const CALCULATION_SETTINGS: &[CalculationSetting] = &[CalculationSetting::IdlePower];

/// Width of the widest settings-bar label, shared so both bars line up.
fn settings_label_width(ui: &Ui) -> f32 {
    let font = TextStyle::Body.resolve(ui.style());
    let color = ui.visuals().text_color();
    let width = |text: &str| {
        ui.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(text.to_owned(), font.clone(), color)
                .size()
                .x
        })
    };
    width("Display")
        .max(width("Research"))
        .max(width("Calculation"))
}

/// Splits settings into rows that fit in `available`, given the gap used
/// between adjacent settings.
fn pack_settings(widths: &[f32], available: f32, between: f32) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    let mut start = 0;
    let mut used = 0.0;
    for (index, &width) in widths.iter().enumerate() {
        if index == start {
            used = width;
        } else if used + between + width <= available {
            used += between + width;
        } else {
            rows.push((start, index));
            start = index;
            used = width;
        }
    }
    if start < widths.len() {
        rows.push((start, widths.len()));
    }
    rows
}

/// Measures one setting in an invisible sizing pass.
fn measure_setting<T: Copy>(
    ui: &Ui,
    label: &str,
    index: usize,
    setting: T,
    setting_ui: &mut impl FnMut(&mut Ui, T),
) -> f32 {
    let mut measure = Ui::new(
        ui.ctx().clone(),
        ui.scope_id().with(("settings_measure", label, index)),
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(10_000.0, 10_000.0),
            ))
            .layout(egui::Layout::left_to_right(Align::Center))
            .style(ui.style().clone())
            .invisible()
            .sizing_pass(),
    );
    setting_ui(&mut measure, setting);
    measure.min_rect().width()
}

/// Lays out a settings bar: label and vertical separator fixed on the left,
/// settings flowing into as many rows as fit on the right. Each row scrolls
/// horizontally if a single setting is wider than the column.
fn settings_bar<T: Copy>(
    ui: &mut Ui,
    label: &str,
    label_width: f32,
    settings: &[T],
    mut setting_ui: impl FnMut(&mut Ui, T),
) {
    let gap = ui.spacing().item_spacing.x;
    let separator = ui.spacing().item_spacing.y;
    let left_width = label_width + 2.0 * gap + separator;
    // A vertical separator widget also gets item spacing on both sides.
    let between = 2.0 * gap + separator;

    let widths: Vec<f32> = settings
        .iter()
        .enumerate()
        .map(|(index, setting)| measure_setting(ui, label, index, *setting, &mut setting_ui))
        .collect();

    let start_x = ui.cursor().min.x;
    let column = ui
        .horizontal(|ui| {
            ui.add_space(left_width);
            let rows = pack_settings(&widths, ui.available_width(), between);
            ui.vertical(|ui| {
                for (row_index, &(start, end)) in rows.iter().enumerate() {
                    if row_index > 0 {
                        ui.add(egui::Separator::default().spacing(separator));
                    }
                    let _row_response = ui.horizontal(|ui| {
                        ui.set_min_height(glyph_button_height(ui));
                        egui::ScrollArea::horizontal()
                            .id_salt(("settings_row", label, row_index))
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                            )
                            .show(ui, |ui| {
                                ui.set_min_height(glyph_button_height(ui));
                                ui.with_layout(egui::Layout::left_to_right(Align::Center), |ui| {
                                    for index in start..end {
                                        if index > start {
                                            ui.add(
                                                egui::Separator::default()
                                                    .vertical()
                                                    .spacing(separator),
                                            );
                                        }
                                        setting_ui(ui, settings[index]);
                                    }
                                });
                            });
                    });
                }
            })
            .response
            .rect
        })
        .inner;

    let label_rect = egui::Rect::from_min_size(
        egui::pos2(start_x, column.top()),
        egui::vec2(label_width, column.height()),
    );
    let mut label_ui = Ui::new(
        ui.ctx().clone(),
        ui.scope_id().with(("settings_label", label)),
        egui::UiBuilder::new()
            .max_rect(label_rect)
            .layout(egui::Layout::left_to_right(Align::Center))
            .style(ui.style().clone()),
    );
    label_ui.strong(label);
    ui.painter().vline(
        start_x + label_width + gap + separator / 2.0,
        column.y_range(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
}

/// The game item whose icon represents a belt tier.
/// The most research levels an effect can take before the productivity cap is
/// reached, which bounds the research slider.
fn productivity_research_cap(research: &hactorio_data::ProductivityResearch) -> u32 {
    const BONUS_CAP: f64 = 3.0;
    research
        .effects
        .iter()
        .filter(|effect| effect.change > 0.0)
        .map(|effect| {
            // Trim float noise so an exact multiple does not round up.
            let levels = BONUS_CAP / effect.change;
            (levels - levels * 1e-12).ceil() as u32
        })
        .max()
        .unwrap_or(u32::MAX)
}

/// The game item whose icon represents a belt tier.
fn belt_tier_glyph(registry: &Registry, tier: BeltTier) -> Option<char> {
    registry.entry(tier.item_id()).map(|entry| entry.glyph)
}

pub struct HactorioApp {
    tabs: Vec<Tab>,
    active_tab: usize,
    settings: Settings,
    evaluator: Evaluator,
    registry: Option<Arc<Registry>>,
    outputs: Evaluation,
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
    /// Monospace font family for the editor and outputs; `None` is the default.
    font: Option<String>,
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
        let active_tab = document.active_tab.min(tabs.len() - 1);
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
            glyphs::install(ctx, Arc::clone(registry), &install.icon_source());
        }

        let font = document
            .font
            .filter(|name| fonts::system_fonts().contains(name));
        fonts::system_fonts().apply(ctx, font.as_deref());

        let zoom = document.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        ctx.set_zoom_factor(zoom);
        let font_size = document.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        set_font_size(ctx, font_size);

        HactorioApp {
            tabs,
            active_tab,
            settings: document.settings,
            evaluator: Evaluator::new(),
            registry,
            outputs: Evaluation::empty(),
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
            font,
            pending_focus: document
                .cursors
                .get(active_tab)
                .map(|cursor| (cursor.line, cursor.column)),
            applied: 0,
            completion: CompletionState::default(),
        }
    }

    fn current_text(&self) -> String {
        self.tabs
            .get(self.active_tab)
            .map(|tab| tab.text.clone())
            .unwrap_or_default()
    }

    fn poll_evaluator(&mut self) {
        while let Some(response) = self.evaluator.poll() {
            if response.generation == self.pending {
                self.outputs = response.evaluation;
                self.applied = response.generation;
            }
        }
    }

    fn maybe_request(&mut self) {
        let Some(registry) = self.registry.clone() else {
            self.outputs = Evaluation::empty();
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
        glyphs::install(ctx, Arc::clone(&registry), &install.icon_source());
        self.registry = Some(registry);
        self.install = Some(install);
        self.evaluator = Evaluator::new();
        self.last_request = None;
        self.outputs = Evaluation::empty();
        self.dirty = true;
    }

    fn tabs_panel(&mut self, ui: &mut Ui) {
        let mut activate = None;
        let mut delete = None;

        egui::Panel::bottom("tabs_actions")
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(8))
            .show(ui, |ui| {
                let loading = self.loading();
                let label = if loading {
                    "Loading game data…".to_string()
                } else {
                    "Reload game data".to_string()
                };
                let size = egui::vec2(ui.available_width(), 30.0);
                ui.vertical_centered(|ui| {
                    if ui
                        .add_enabled(!loading, egui::Button::new(label).min_size(size))
                        .clicked()
                    {
                        self.start_load();
                    }
                })
            });

        egui::ScrollArea::vertical()
            .id_salt("tabs_list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let h = 40.0;

                egui::Frame::new()
                    .inner_margin(8)
                    .show(ui, |ui| ui.heading("Pages"));

                ui.spacing_mut().item_spacing.y = 0.0;

                for (index, tab) in self.tabs.iter().enumerate() {
                    let selected = index == self.active_tab;
                    let button_text = egui::RichText::new(tab.title()).size(14.0);
                    let r = ui.add(
                        egui::Button::selectable(selected, button_text)
                            .min_size(egui::vec2(ui.available_width(), h))
                            .corner_radius(0)
                            .truncate(),
                    );
                    if r.middle_clicked() {
                        delete = Some(index);
                    } else if r.clicked() {
                        activate = Some(index);
                    }
                    r.context_menu(|ui| {
                        if ui.button("🗑️ Delete page").clicked() {
                            delete = Some(index);
                        }
                    });
                }

                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if ui
                        .add(
                            egui::Button::new(egui::Atom::paint(
                                egui::vec2(14.0, 14.0),
                                |ui, atom_paint_args| {
                                    let rect = atom_paint_args.rect;
                                    let color = atom_paint_args.fallback_text_color;
                                    ui.painter().line_segment(
                                        [rect.center_top(), rect.center_bottom()],
                                        (1.0, color),
                                    );
                                    ui.painter().line_segment(
                                        [rect.left_center(), rect.right_center()],
                                        (1.0, color),
                                    );
                                },
                            ))
                            .min_size(egui::vec2(32.0, 32.0)),
                        )
                        .on_hover_text("New page")
                        .clicked()
                        || ui.input_mut(|i| {
                            i.consume_shortcut(&egui::KeyboardShortcut::new(
                                egui::Modifiers::COMMAND,
                                egui::Key::T,
                            ))
                        })
                    {
                        self.tabs.push(Tab::from_text("# Untitled\n"));
                        self.active_tab = self.tabs.len() - 1;
                        self.dirty = true;
                        self.last_request = None;
                    }
                });
            });

        if let Some(index) = activate
            && index != self.active_tab
        {
            self.active_tab = index;
            self.outputs = Evaluation::empty();
            self.last_request = None;
        }
        if let Some(index) = delete {
            self.tabs.remove(index);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            self.dirty = true;
            self.last_request = None;
        }
    }

    fn display_bar(&mut self, ui: &mut Ui, registry: &Registry, label_width: f32) {
        let before = self.settings.clone();

        settings_bar(
            ui,
            "Display",
            label_width,
            DISPLAY_SETTINGS,
            |ui, setting| self.display_setting_ui(ui, setting, registry),
        );

        self.settings_changed(before);
    }

    fn display_setting_ui(&mut self, ui: &mut Ui, setting: DisplaySetting, registry: &Registry) {
        match setting {
            DisplaySetting::Theme => {
                ui.label("Theme");
                egui::global_theme_preference_buttons(ui);
            }
            DisplaySetting::Font => {
                ui.label("Font");
                let mut font = self.font.clone();
                egui::ComboBox::from_id_salt("editor_font")
                    .selected_text(font.as_deref().unwrap_or("Default"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut font, None, "Default");
                        for family in fonts::system_fonts().families() {
                            ui.selectable_value(&mut font, Some(family.clone()), family);
                        }
                    });
                if font != self.font {
                    self.font = font;
                    fonts::system_fonts().apply(ui.ctx(), self.font.as_deref());
                    self.dirty = true;
                }
                let font_size = ui.add(
                    egui::DragValue::new(&mut self.font_size)
                        .range(MIN_FONT_SIZE..=MAX_FONT_SIZE)
                        .speed(0.1)
                        .fixed_decimals(0),
                );
                if font_size.changed() {
                    set_font_size(ui.ctx(), self.font_size);
                    self.dirty = true;
                }
            }
            DisplaySetting::TimeUnit => {
                ui.label("Time unit");
                for unit in [TimeUnit::Second, TimeUnit::Minute, TimeUnit::Hour] {
                    ui.selectable_value(&mut self.settings.time_unit, unit, unit.name());
                }
            }
            DisplaySetting::ItemRates => {
                ui.label("Item rates");
                let unit = self.settings.time_unit.short_name();
                ui.selectable_value(
                    &mut self.settings.item_rate_belt_display,
                    None,
                    format!("item/{unit}"),
                );
                for tier in BeltTier::ALL {
                    let label: egui::WidgetText = match belt_tier_glyph(registry, tier) {
                        Some(glyph) => RichText::new(glyph.to_string()).size(GLYPH_SIZE).into(),
                        None => tier.item_id().to_string().into(),
                    };
                    ui.selectable_value(
                        &mut self.settings.item_rate_belt_display,
                        Some(tier),
                        label,
                    )
                    .on_hover_text(format!("{} item/s", tier.items_per_second()));
                }
                ui.add_enabled_ui(self.settings.item_rate_belt_display.is_some(), |ui| {
                    let stack_label =
                        RichText::new(hactorio_data::STACK_SIZE_GLYPH.to_string()).size(GLYPH_SIZE);
                    ui.toggle_value(&mut self.settings.stacked_belts_display, stack_label)
                        .on_hover_text("Show item rates as stacks on stacked belts");
                });
            }
            DisplaySetting::Decimals => {
                ui.label("Decimals");
                ui.add(egui::Slider::new(&mut self.settings.max_decimals, 0..=6));
            }
            DisplaySetting::MetricPrefixes => {
                ui.checkbox(&mut self.settings.metric_prefixes, "Metric prefixes");
            }
        }
    }

    fn research_bar(&mut self, ui: &mut Ui, registry: &Registry, label_width: f32) {
        let before = self.settings.clone();
        let researches = registry.productivity_researches();

        settings_bar(
            ui,
            "Research",
            label_width,
            RESEARCH_SETTINGS,
            |ui, setting| self.research_setting_ui(ui, setting, registry, researches),
        );

        self.settings_changed(before);
    }

    fn research_setting_ui(
        &mut self,
        ui: &mut Ui,
        setting: ResearchSetting,
        registry: &Registry,
        researches: &[hactorio_data::ProductivityResearch],
    ) {
        match setting {
            ResearchSetting::BeltStacking => {
                ui.label("Belt stacking");
                ui.add(egui::Slider::new(&mut self.settings.belt_stacking, 1..=4));
            }
            ResearchSetting::InserterCapacity => {
                ui.label("Inserter capacity");
                ui.add(egui::Slider::new(
                    &mut self.settings.inserter_capacity,
                    0..=7,
                ));
            }
            ResearchSetting::Productivity => {
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
            }
        }
    }

    fn calculation_bar(&mut self, ui: &mut Ui, label_width: f32) {
        let before = self.settings.clone();

        settings_bar(
            ui,
            "Calculation",
            label_width,
            CALCULATION_SETTINGS,
            |ui, setting| self.calculation_setting_ui(ui, setting),
        );

        self.settings_changed(before);
    }

    fn calculation_setting_ui(&mut self, ui: &mut Ui, setting: CalculationSetting) {
        match setting {
            CalculationSetting::IdlePower => {
                ui.label("Idle power");
                ui.selectable_value(&mut self.settings.idle_power, IdlePower::Ignored, "Ignored");
                ui.selectable_value(
                    &mut self.settings.idle_power,
                    IdlePower::Fractional,
                    "Fractional",
                );
                ui.selectable_value(
                    &mut self.settings.idle_power,
                    IdlePower::RoundedUp,
                    "Rounded up",
                )
                .on_hover_text(
                    "Include idle power for the number of machines needed, rounded up to the next integer",
                );
            }
        }
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
                let cap = productivity_research_cap(research);
                let mut level = self.settings.research_level(&research.id).min(cap);
                let drag = ui.horizontal(|ui| {
                    let drag = ui.add(egui::DragValue::new(&mut level).range(0..=cap).speed(0.1));
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
            active_tab: self.active_tab,
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
            font: self.font.clone(),
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
        if let Some(registry) = &self.registry {
            ctx.input_mut(|input| clipboard::translate_input(input, registry));
        }
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
        let label_width = settings_label_width(ui);
        if let Some(registry) = &loaded {
            egui::Panel::top("display_settings").show(ui, |ui| {
                self.display_bar(ui, registry, label_width);
            });

            egui::Panel::top("research_settings").show(ui, |ui| {
                self.research_bar(ui, registry, label_width);
            });

            egui::Panel::top("calculation_settings").show(ui, |ui| {
                self.calculation_bar(ui, label_width);
            });

            egui::Panel::bottom("status").show(ui, |ui| {
                let sp = std::mem::take(&mut ui.spacing_mut().item_spacing);
                ui.horizontal(|ui| {
                    egui::ScrollArea::horizontal()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            show_credits(ui);
                            ui.add_space(sp.x);
                            ui.separator();
                            ui.add_space(sp.x);
                            show_powered_by_egui(ui);
                            ui.add_space(sp.x);
                            ui.separator();
                            ui.add_space(sp.x);
                            show_source_code_link(ui);
                        });
                });
            });

            egui::Panel::left("tabs")
                .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(0))
                .default_size(240.0)
                .show(ui, |ui| {
                    self.tabs_panel(ui);
                });
        }

        egui::CentralPanel::default().show(ui, |ui| {
            let Some(registry) = self.registry.clone() else {
                ui.vertical_centered(|ui| {
                    ui.add_space(48.0);
                    ui.label("Game data is not loaded yet.");
                    let loading = self.loading();
                    let label = if loading {
                        "Loading game data…"
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
            let active_tab = self.active_tab;
            let mut state = EditorState::new(
                active_tab,
                &self.outputs,
                &self.settings,
                registry,
                &mut self.pending_focus,
                &mut self.completion,
                &mut self.results_width,
            );
            let events = editor_ui(ui, &mut self.tabs[active_tab], &mut state);
            if events.changed {
                self.dirty = true;
                self.last_request = None;
            }
            if events.results_resized {
                self.dirty = true;
            }
            let results = results::ResultsContext {
                tabs: &self.tabs,
                active_tab,
                outputs: &self.outputs,
                settings: &self.settings,
                registry,
            };
            if let Some((line, anchor, width)) = events.multiline_result
                && self.completion.items.is_empty()
            {
                results.show_multiline_result(&ctx, anchor, line, width);
            }
            if let Some(token) = events.hover_token {
                results.show_token_tip(&ctx, token);
            }
        });

        if self.dirty && self.last_save.elapsed() >= SAVE_INTERVAL {
            self.save();
        }

        // Last, so the commands cover every widget drawn this frame.
        if let Some(registry) = &self.registry {
            ctx.output_mut(|output| clipboard::translate_output(output, registry));
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

fn show_credits(ui: &mut egui::Ui) {
    ui.label(format!("{APP_TITLE} v{} by ", env!("CARGO_PKG_VERSION")));
    ui.hyperlink_to("Andrew Farkas", "https://ajfarkas.dev/");
}

fn show_powered_by_egui(ui: &mut egui::Ui) {
    ui.label("Powered by ");
    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
    ui.label(" and ");
    ui.hyperlink_to(
        "eframe",
        "https://github.com/emilk/egui/tree/master/crates/eframe",
    );
}

fn show_source_code_link(ui: &mut egui::Ui) {
    ui.hyperlink_to(
        egui::RichText::new(" source code").small(),
        env!("CARGO_PKG_REPOSITORY"),
    );
}

#[cfg(test)]
mod tests;
