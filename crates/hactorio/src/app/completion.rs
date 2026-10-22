//! Autocomplete: word extraction, aliases, fuzzy ranking, and the popup.

use egui::{Key, Modifiers, TextStyle, Ui};
use hactorio_data::{EntryId, Registry};

/// Maximum height of the autocomplete popup, before it scrolls.
pub(super) const COMPLETION_MAX_HEIGHT: f32 = 240.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionContext {
    Normal,
    Machine,
    Recipe,
}

#[derive(Clone, Default)]
pub(super) struct CompletionState {
    /// Character index of the caret when completions were last updated.
    pub(super) cursor: usize,
    pub(super) word: String,
    pub(super) items: Vec<CompletionItem>,
    pub(super) selected: usize,
    pub(super) dismissed: Option<String>,
    /// Whether the next render should scroll the selected row into view.
    pub(super) needs_scroll: bool,
    /// The selected row's rect from the last render.
    #[cfg(test)]
    pub(super) selected_rect: Option<egui::Rect>,
}

#[derive(Clone)]
pub(super) struct CompletionItem {
    pub(super) insert: String,
    /// Icon glyph; variables use [`crate::icons::VARIABLE`].
    pub(super) glyph: char,
    /// The name or alias that matched best, as shown in the popup.
    pub(super) label: String,
    /// Character indices in `label` that matched the query.
    pub(super) matched: Vec<usize>,
    /// The localized prototype name, when it differs from `label`.
    pub(super) name: Option<String>,
    /// Character indices in `name` that matched the query.
    pub(super) name_matched: Vec<usize>,
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
pub(super) fn completion_word(text: &str, cursor: usize) -> (usize, String) {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let mut start = cursor;
    while start > 0 && !is_completion_boundary(chars[start - 1]) {
        start -= 1;
    }

    // `-` separates words in arithmetic, but aliases and prototype IDs contain
    // it. Rejoin the dash-separated segments so `green-chip` and
    // `speed-mod-2` are matched (and replaced) whole, but only when the joined
    // token starts with a letter so arithmetic such as `2-3` stays separate.
    let mut token_start = start;
    while token_start >= 2
        && chars[token_start - 1] == '-'
        && !is_completion_boundary(chars[token_start - 2])
    {
        token_start -= 1;
        while token_start > 0 && !is_completion_boundary(chars[token_start - 1]) {
            token_start -= 1;
        }
    }
    let starts_word = match chars.get(token_start) {
        Some(ch) if ch.is_ascii_alphabetic() => true,
        Some(':') => chars
            .get(token_start + 1)
            .is_some_and(|ch| ch.is_ascii_alphabetic()),
        _ => false,
    };
    if starts_word {
        start = token_start;
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
    ("speed-module", &["speed-mod-1"]),
    ("speed-module-2", &["speed-mod-2"]),
    ("speed-module-3", &["speed-mod-3"]),
    ("productivity-module", &["prod-mod-1"]),
    ("productivity-module-2", &["prod-mod-2"]),
    ("productivity-module-3", &["prod-mod-3"]),
    ("efficiency-module", &["eff-mod-1"]),
    ("efficiency-module-2", &["eff-mod-2"]),
    ("efficiency-module-3", &["eff-mod-3"]),
    ("quality-module", &["qual-mod-1"]),
    ("quality-module-2", &["qual-mod-2"]),
    ("quality-module-3", &["qual-mod-3"]),
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

/// Ranking bonus for a variable bound in the current tab.
///
/// Variables are few and user-authored, so a comparable match should outrank a
/// prototype ID. The bonus beats the popularity bonus of common prototypes
/// without overriding a clearly better match (word starts score 72 more).
const COMPLETION_VARIABLE_BONUS: f64 = 20.0;

/// The ranking key of a completion: its fuzzy score plus a popularity bonus.
fn completion_rank(score: isize, references: u32) -> f64 {
    score as f64 + f64::from(references).ln_1p() * COMPLETION_POPULARITY_WEIGHT
}

/// Bonus for a name that equals the needle, case-insensitively.
///
/// Short exact names such as `pm2` can score below a multi-word name whose
/// word starts earn more points. Exact matches should win those near-ties.
const COMPLETION_EXACT_MATCH_BONUS: isize = 50;

/// Fuzzy scoring weights tuned for prototype names.
///
/// The defaults let a contiguous prefix outrank a match that starts words:
/// for `castir`, `casting-copper` (`CASTI…r`) beat `casting-iron` (`CAST`+`IR`)
/// because the longer consecutive run outweighed the word-start bonus. Lowering
/// the consecutive bonus and raising the distance penalty makes the jump to
/// `iron` win without turning every multi-word name into a match.
const COMPLETION_SCORING: sublime_fuzzy::Scoring = sublime_fuzzy::Scoring {
    bonus_consecutive: 4,
    bonus_word_start: 72,
    bonus_match_case: 8,
    penalty_distance: 6,
};

/// Fuzzy score of `needle` against `name`, with the matched char indices.
fn fuzzy_match(name: &str, needle: &str) -> Option<(isize, Vec<usize>)> {
    let found = sublime_fuzzy::FuzzySearch::new(needle, name)
        .case_insensitive()
        .score_with(&COMPLETION_SCORING)
        .best_match()?;
    let mut matched: Vec<usize> = found.matched_indices().copied().collect();
    matched.sort_unstable();
    Some((found.score(), matched))
}

/// The name of `entry` that best matches `needle`, with the matched char indices
/// and whether the localized display name was the best match.
pub(super) fn best_name_match<'a>(
    entry: &'a hactorio_data::Entry,
    needle: &str,
) -> Option<(isize, &'a str, Vec<usize>, bool)> {
    let display_name = entry.display_name.as_deref();
    std::iter::once(entry.id.as_ref())
        .chain(display_name)
        .chain(aliases_for(&entry.id).iter().copied())
        .filter_map(|name| {
            let (score, matched) = fuzzy_match(name, needle)?;
            let score = score
                + if name.eq_ignore_ascii_case(needle) {
                    COMPLETION_EXACT_MATCH_BONUS
                } else {
                    0
                };
            Some((score, name, matched, Some(name) == display_name))
        })
        .reduce(|best, candidate| {
            if candidate.0 > best.0 {
                candidate
            } else {
                best
            }
        })
}

/// The variable name defined by `line`, if it is a definition.
pub(super) fn definition_name(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let first = line.chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut end = 0;
    for (index, ch) in line.char_indices() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            end = index + 1;
        } else {
            break;
        }
    }
    let rest = line[end..].trim_start();
    let after_equals = rest.strip_prefix('=')?;
    if after_equals.starts_with('=') {
        return None;
    }
    Some(&line[..end])
}

/// Variables defined on lines before the line `before` ends on.
fn defined_variables(before: &[char]) -> Vec<String> {
    let text: String = before.iter().collect();
    let mut lines: Vec<&str> = text.split('\n').collect();
    lines.pop();
    let mut names: Vec<String> = Vec::new();
    for name in lines.iter().filter_map(|line| definition_name(line)) {
        if !names.iter().any(|known| known == name) {
            names.push(name.to_string());
        }
    }
    names
}

pub(super) fn completion_items(
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

    if context == CompletionContext::Normal && !word.starts_with(':') {
        for name in defined_variables(&before) {
            if registry.entry(&name).is_some() {
                continue;
            }
            let Some((score, matched)) = fuzzy_match(&name, needle) else {
                continue;
            };
            items.push((
                score as f64 + COMPLETION_VARIABLE_BONUS,
                CompletionItem {
                    insert: name.clone(),
                    glyph: crate::icons::VARIABLE,
                    label: name,
                    matched,
                    name: None,
                    name_matched: Vec::new(),
                },
            ));
        }
    }

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
pub(super) const COMPLETION_NAME_GAP: usize = 4;

/// Number of padding spaces before the prototype name, for column alignment.
fn completion_padding(item: &CompletionItem, label_width: usize) -> usize {
    if item.name.is_some() {
        label_width.saturating_sub(item.label.chars().count()) + COMPLETION_NAME_GAP
    } else {
        0
    }
}

/// Appends the icon column: the row's glyph and its separating space.
fn append_icon_column(
    job: &mut egui::text::LayoutJob,
    item: &CompletionItem,
    normal: &egui::TextFormat,
) {
    job.append(&item.glyph.to_string(), 0.0, normal.clone());
    job.append(" ", 0.0, normal.clone());
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
pub(super) fn completion_layout_job(
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
    append_icon_column(&mut job, item, &normal);
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

/// Keys the popup owns while it is open.
///
/// They are consumed before the text editor sees them, so Tab and Enter pick a
/// row instead of editing text.
#[derive(Clone, Copy, Default)]
pub(super) struct PopupKeys {
    pub accept: bool,
    pub dismiss: bool,
    pub move_selection: i32,
}

/// Consumes the keys the popup handles.
pub(super) fn popup_keys(ui: &mut Ui) -> PopupKeys {
    let mut keys = PopupKeys::default();
    let mut tab_accept = false;
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
        keys.accept = input.consume_key(Modifiers::NONE, Key::Enter) || tab_accept;
        keys.dismiss = input.consume_key(Modifiers::NONE, Key::Escape);
        keys.move_selection = down - up;
    });
    if tab_accept {
        // Keep Tab from moving focus; it accepts the suggestion instead.
        ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
    }
    keys
}

impl CompletionState {
    /// Whether the popup should be shown, unless it was dismissed for this word.
    pub(super) fn is_open(&self) -> bool {
        !self.items.is_empty() && self.dismissed.as_deref() != Some(self.word.as_str())
    }
}

/// What the editor hands to the popup for one frame.
pub(super) struct PopupRequest<'a> {
    pub active: usize,
    pub text_id: egui::Id,
    pub text: &'a mut String,
    pub completion: &'a mut CompletionState,
    /// Character index where the word being completed starts.
    pub word_start: usize,
    /// Where the popup's top-left corner would like to be.
    pub anchor: egui::Pos2,
    pub keys: PopupKeys,
}

/// Shows the popup and applies the chosen completion. Returns whether the text
/// changed and the editor must be saved.
pub(super) fn popup(ui: &mut Ui, request: PopupRequest<'_>) -> bool {
    let PopupRequest {
        active,
        text_id,
        text,
        completion,
        word_start,
        anchor,
        keys,
    } = request;

    if completion.items.is_empty() {
        return false;
    }
    if keys.dismiss {
        completion.dismissed = Some(completion.word.clone());
        completion.items.clear();
        return false;
    }
    if keys.move_selection != 0 {
        let len = completion.items.len() as i32;
        completion.selected =
            (completion.selected as i32 + keys.move_selection).rem_euclid(len) as usize;
        completion.needs_scroll = true;
    }
    if keys.accept {
        insert(
            ui,
            text_id,
            text,
            completion,
            word_start,
            completion.selected,
        );
        return true;
    }

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
                    let job =
                        completion_layout_job(item, label_width, false, &font_id, ui.visuals());
                    ui.fonts_mut(|fonts| fonts.layout_job(job).size().x)
                })
                .fold(0.0, f32::max);
            let chrome = egui::Frame::popup(style);
            let chrome_width = chrome.inner_margin.sum().x + 2.0 * chrome.stroke.width;
            let content_width = (text_width
                + 2.0 * style.spacing.button_padding.x
                + style.spacing.scroll.allocated_width())
            .max(300.0);
            let total_width =
                (content_width + chrome_width).min(ui.ctx().viewport_rect().width() - 16.0);
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
                        for (index, item) in completion.items.iter().enumerate() {
                            let job = completion_layout_job(
                                item,
                                label_width,
                                index == selected,
                                &font_id,
                                ui.visuals(),
                            );
                            let response = ui.selectable_label(index == selected, job);
                            if response.clicked() {
                                clicked = Some(index);
                            }
                            #[cfg(test)]
                            if index == selected {
                                completion.selected_rect = Some(response.rect);
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
        insert(ui, text_id, text, completion, word_start, index);
        return true;
    }
    false
}

/// Replaces the completed word, restores the caret, and closes the popup.
fn insert(
    ui: &mut Ui,
    text_id: egui::Id,
    text: &mut String,
    completion: &mut CompletionState,
    word_start: usize,
    index: usize,
) {
    let inserted = insert_completion(
        text,
        word_start,
        completion.word.chars().count(),
        &completion.items[index].insert,
    );
    if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), text_id) {
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
}
