//! The tab editor: one multiline text field with outputs painted alongside it.

use egui::{
    Color32, Context, CursorIcon, Rect, ScrollArea, Sense, TextEdit, TextStyle, Ui,
};
use hactorio_core::Settings;
use hactorio_core::facet_eval::{LineOutput, StatementOutput, line_hash};
use hactorio_data::Registry;

use super::completion::{self, CompletionState, completion_items, completion_word};
use super::results::{Token, glyph_at, identifier_at, single_line_result};
use crate::document::Tab;

/// Whether a floating surface is covering `pointer`, so editors below it do not react.
fn pointer_over_floating(ctx: &Context, active_tab: usize, pointer: egui::Pos2) -> bool {
    [
        egui::Id::unique("multiline_result"),
        egui::Id::unique(("completion", active_tab)),
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

pub(super) struct EditorState<'a> {
    active_tab: usize,
    outputs: &'a [StatementOutput],
    settings: &'a Settings,
    registry: &'a Registry,
    pending_focus: &'a mut Option<(usize, usize)>,
    completion: &'a mut CompletionState,
    results_width: &'a mut f32,
}

impl<'a> EditorState<'a> {
    pub(super) fn new(
        active_tab: usize,
        outputs: &'a [StatementOutput],
        settings: &'a Settings,
        registry: &'a Registry,
        pending_focus: &'a mut Option<(usize, usize)>,
        completion: &'a mut CompletionState,
        results_width: &'a mut f32,
    ) -> Self {
        EditorState {
            active_tab,
            outputs,
            settings,
            registry,
            pending_focus,
            completion,
            results_width,
        }
    }
}

pub(super) struct EditorEvents {
    pub(super) changed: bool,
    /// The results column was resized.
    pub(super) results_resized: bool,
    /// The line the caret is on, while the editor has focus.
    focused_line: Option<usize>,
    /// Where to place the multiline result panel, the line it describes, and its width.
    pub(super) multiline_result: Option<(usize, egui::Pos2, f32)>,
    /// The token the pointer is over.
    pub(super) hover_token: Option<Token>,
}

/// Wraps the editor's selection in parentheses when `(` is typed, instead of
/// letting the character replace the selection.
///
/// The selection stays on the wrapped text so that another bracket typed
/// immediately wraps it again. Returns whether the text changed.
fn wrap_selection_in_parens(ctx: &Context, text_id: egui::Id, text: &mut String) -> bool {
    if !ctx.memory(|memory| memory.has_focus(text_id)) {
        return false;
    }
    let Some(mut state) = egui::text_edit::TextEditState::load(ctx, text_id) else {
        return false;
    };
    let Some(range) = state.cursor.char_range().filter(|range| !range.is_empty()) else {
        return false;
    };
    // Only take the `(` when it is actually being typed; otherwise leave it for the
    // text edit.
    let mut typed = false;
    ctx.input_mut(|input| {
        input.events.retain(|event| match event {
            egui::Event::Text(text) if text == "(" => {
                typed = true;
                false
            }
            _ => true,
        });
    });
    if !typed {
        return false;
    }

    let range = range.as_sorted_char_range();
    let start = range.start.0;
    let end = range.end.0;
    let start_byte = text
        .char_indices()
        .nth(start)
        .map_or(text.len(), |(index, _)| index);
    let end_byte = text
        .char_indices()
        .nth(end)
        .map_or(text.len(), |(index, _)| index);
    text.insert(end_byte, ')');
    text.insert(start_byte, '(');
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start + 1),
            egui::text::CCursor::new(end + 1),
        )));
    state.store(ctx, text_id);
    true
}

/// Renders the tab as one multiline editor, with line numbers and outputs
/// painted at the galley's row positions so they stay aligned.
pub(super) fn editor_ui(ui: &mut Ui, tab: &mut Tab, state: &mut EditorState<'_>) -> EditorEvents {
    let active_tab = state.active_tab;
    let outputs = state.outputs;
    let settings = state.settings;
    let registry = state.registry;
    let pending_focus: &mut Option<(usize, usize)> = state.pending_focus;
    let completion: &mut CompletionState = state.completion;
    let results_width: &mut f32 = state.results_width;

    let mut events = EditorEvents {
        changed: false,
        results_resized: false,
        focused_line: None,
        multiline_result: None,
        hover_token: None,
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
    let line_numbers_width = 44.0;
    let results_divider_width = 7.0;
    let central_width = ui.available_width();
    let max_results_width = (central_width - line_numbers_width - 220.0).max(120.0);
    let requested_results_width = (*results_width).clamp(120.0, max_results_width);
    let editor_width =
        (central_width - line_numbers_width - requested_results_width - results_divider_width)
            .max(160.0);
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
        .id_salt(("editor", active_tab))
        .vertical_scroll_offset(tab.scroll)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add_space(line_numbers_width);
                let popup_open = completion.is_open();
                let keys = if popup_open {
                    completion::popup_keys(ui)
                } else {
                    completion::PopupKeys::default()
                };

                let text_id = ui.make_persistent_id(("text", active_tab));
                // Clicking the popup must not surrender the editor's focus, or the
                // popup would be cleared before the click reaches its row.
                let popup_id = egui::Id::unique(("completion", active_tab));
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
                let mut wrapped = false;
                let hscroll = ScrollArea::horizontal()
                    .id_salt(("text_scroll", active_tab))
                    .max_width(editor_width)
                    .show(ui, |ui| {
                        wrapped = wrap_selection_in_parens(ui.ctx(), text_id, &mut tab.text);
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
                events.changed |= wrapped || text_edit.response.changed();

                if let Some(pointer) = ui.ctx().pointer_hover_pos()
                    && text_edit.response.rect.contains(pointer)
                    && !pointer_over_floating(ui.ctx(), active_tab, pointer)
                {
                    if let Some((_, glyph, rect)) =
                        glyph_at(registry, &text_edit.galley, text_edit.galley_pos, pointer)
                    {
                        events.hover_token = Some(Token::Prototype { glyph, rect });
                    } else if let Some((line, name, rect)) =
                        identifier_at(&text_edit.galley, text_edit.galley_pos, pointer)
                    {
                        events.hover_token = Some(Token::Variable { line, name, rect });
                    }
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
                        events.focused_line = Some(line);
                    }
                }

                let cursor_char = text_edit
                    .cursor_range
                    .map(|range| range.primary.index.0)
                    .unwrap_or(0);
                let (word_start, word) = completion_word(&tab.text, cursor_char);
                let after_dot = tab.text.chars().take(cursor_char).last() == Some('.');
                let typed = text_edit.response.changed();
                if !text_edit.response.has_focus() {
                    completion.items.clear();
                } else if popup_open && !typed && cursor_char != completion.cursor {
                    // The caret moved without typing: close the popup.
                    completion.items.clear();
                }

                if typed && text_edit.response.has_focus() {
                    completion.word = word.clone();
                    completion.selected = 0;
                    completion.needs_scroll = true;
                    completion.items = if word.is_empty() && !after_dot {
                        Vec::new()
                    } else {
                        completion_items(registry, settings, &tab.text, word_start, &word)
                    };
                    if completion.dismissed.as_deref() != Some(word.as_str()) {
                        completion.dismissed = None;
                    }
                }
                completion.cursor = cursor_char;

                if popup_open {
                    let caret = text_edit
                        .galley
                        .pos_from_cursor(egui::text::CCursor::new(cursor_char));
                    let anchor =
                        text_edit.galley_pos + caret.left_bottom().to_vec2() + egui::vec2(0.0, 2.0);
                    events.changed |= completion::popup(
                        ui,
                        completion::PopupRequest {
                            active_tab,
                            text_id,
                            text: &mut tab.text,
                            completion,
                            word_start,
                            anchor,
                            keys,
                        },
                    );
                }

                let viewport = hscroll.inner_rect;
                let origin = text_edit.galley_pos;
                let galley = &text_edit.galley;
                let editor_height = text_edit.response.rect.height().max(galley.size().y + 8.0);

                let (results_divider_rect, results_divider) = ui.allocate_exact_size(
                    egui::vec2(results_divider_width, editor_height),
                    Sense::drag(),
                );
                if results_divider.hovered() || results_divider.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                if results_divider.dragged() {
                    *results_width = (*results_width - results_divider.drag_delta().x)
                        .clamp(120.0, max_results_width);
                    events.results_resized = true;
                }
                let shown_results_width = (*results_width).clamp(120.0, max_results_width);
                ui.painter().vline(
                    results_divider_rect.center().x,
                    results_divider_rect.y_range(),
                    ui.visuals().widgets.noninteractive.bg_stroke,
                );
                let results_rect = Rect::from_min_size(
                    egui::pos2(results_divider_rect.right(), results_divider_rect.top()),
                    egui::vec2(shown_results_width, editor_height),
                );
                ui.allocate_space(egui::vec2(shown_results_width, editor_height));

                let single_line_x = results_rect.left() + 8.0;
                let single_line_width = (shown_results_width - 16.0).max(60.0);
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

                    let Some(statement) = outputs.get(index) else {
                        continue;
                    };
                    if statement.source_hash != line_hash(line) {
                        continue;
                    }
                    let Some(text) = single_line_result(statement, settings, registry) else {
                        continue;
                    };
                    if events.focused_line == Some(index) {
                        events.multiline_result =
                            Some((index, egui::pos2(single_line_x, y), single_line_width));
                        continue;
                    }
                    let color = if matches!(statement.output, LineOutput::Error(_)) {
                        error_color
                    } else {
                        weak
                    };
                    let text = truncate_text(painter, &text, &font_id, single_line_width);
                    let pos = egui::pos2(single_line_x, y);
                    let galley = painter.layout_no_wrap(text, font_id.clone(), color);
                    painter.galley(pos, galley.clone(), color);
                    if let Some(pointer) = ui.ctx().pointer_hover_pos()
                        && !pointer_over_floating(ui.ctx(), active_tab, pointer)
                        && let Some((_, glyph, rect)) = glyph_at(registry, &galley, pos, pointer)
                    {
                        events.hover_token = Some(Token::Prototype { glyph, rect });
                    }
                }
            });
        });
    tab.scroll = scroll.state.clamped_offset().y;

    events
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
