//! Autocomplete: word extraction, aliases, fuzzy ranking, and the popup.

use std::collections::HashMap;

use egui::{Key, Modifiers, TextStyle, Ui};
use hactorio_core::Settings;
use hactorio_core::new_eval::{
    Item, Output, Quality, Resource, Value, definition_value_span, evaluate, process_resources,
    property_is_available,
};
use hactorio_data::{EMPTY_GLYPH, Entry, EntryId, Registry, SIGNALS};

/// Maximum height of the autocomplete popup, before it scrolls.
pub(super) const COMPLETION_MAX_HEIGHT: f32 = 240.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionContext {
    Normal,
    Machine,
    Recipe,
    Config,
    Property,
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
    /// The popup's measured width, recomputed whenever the items change.
    metrics: Option<PopupMetrics>,
    /// The selected row's rect from the last render.
    #[cfg(test)]
    pub(super) selected_rect: Option<egui::Rect>,
}

#[derive(Clone)]
pub(super) struct CompletionItem {
    pub(super) insert: String,
    /// Glyphs; variables use [`crate::glyphs::VARIABLE`].
    ///
    /// Most rows show one glyph. A stacked belt row shows the `stacked-belt`
    /// prefix followed by its belt.
    pub(super) glyphs: String,
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
            '+' | '-' | '*' | '/' | '^' | '=' | '(' | ')' | '#' | '.'
        )
}

fn is_pua(ch: char) -> bool {
    (0xE000..=0xF8FF).contains(&(ch as u32))
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

    // A completion may start right after a digit or prototype glyph.
    let mut word: String = chars[start..cursor].iter().collect();
    let leading = word
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || is_pua(*ch))
        .count();
    if leading > 0 && leading < word.chars().count() {
        start += leading;
        word = word.chars().skip(leading).collect();
    }

    (start, word)
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
    ("underground-belt", &["yellow-underground-belt"]),
    ("fast-underground-belt", &["red-underground-belt"]),
    ("express-underground-belt", &["blue-underground-belt"]),
    ("turbo-underground-belt", &["green-underground-belt"]),
    ("splitter", &["yellow-splitter"]),
    ("fast-splitter", &["red-splitter"]),
    ("express-splitter", &["blue-splitter"]),
    ("turbo-splitter", &["green-splitter"]),
    ("atomic-bomb", &["nuclear-bomb", "nuke"]),
    ("lubricant", &["lube"]),
    (
        "firearm-magazine",
        &["yellow-ammo", "yellow-rounds-magazine"],
    ),
    (
        "piercing-rounds-magazine",
        &["red-ammo", "red-rounds-magazine", "piercing-ammo"],
    ),
    (
        "uranium-rounds-magazine",
        &["green-ammo", "green-rounds-magazine"],
    ),
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
    (
        "casting-iron",
        &["iron-plate-casting", "casting-iron-plate"],
    ),
    ("casting-pipe", &["pipe-casting"]),
    ("casting-steel", &["steel-casting"]),
    (
        "casting-copper-plate",
        &["copper-plate-casting", "casting-copper-plate"],
    ),
    ("casting-iron-stick", &["iron-stick-casting"]),
    ("casting-copper-cable", &["copper-cable-casting"]),
    ("casting-pipe-to-ground", &["pipe-to-ground-casting"]),
    ("casting-iron-gear-wheel", &["iron-gear-wheel-casting"]),
    (
        "casting-low-density-structure",
        &["low-density-structure-casting"],
    ),
    ("storage-tank", &["fluid-tank"]),
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
/// word starts earn more points. A dominant bonus keeps an exact match ahead of
/// every fuzzy near-tie, including the present-unit bonus.
const COMPLETION_EXACT_MATCH_BONUS: isize = 1000;

/// Bonus for a name whose first character is the needle's first character.
///
/// `gb` matches both `green-belt` and `petroleum-gas-barrel` at word starts.
/// The initial letters are what a player types, so they break that tie.
const COMPLETION_FIRST_CHAR_BONUS: isize = 30;

/// Whether `name` and `needle` start with the same character, ignoring case.
fn first_char_matches(name: &str, needle: &str) -> bool {
    match (name.chars().next(), needle.chars().next()) {
        (Some(name), Some(needle)) => name.eq_ignore_ascii_case(&needle),
        _ => false,
    }
}

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

/// Whether `name` is `id` with different separators or capitalization.
///
/// Most display names are their prototype ID with `-` replaced by spaces and
/// each word capitalized, and a completion word can never contain a space.
/// Matching such a name a second time can only reach letters the ID already
/// has, so it doubles the fuzzy searches without finding a different row.
fn reforms_id(id: &str, name: &str) -> bool {
    fn letters(text: &str) -> impl Iterator<Item = char> + '_ {
        text.chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .map(|ch| ch.to_ascii_lowercase())
    }
    letters(id).eq(letters(name))
}

/// Scores `name` against `needle`, including the exact and first-character
/// bonuses every spelling of a symbol competes for.
fn name_match(name: &str, needle: &str) -> Option<(isize, Vec<usize>)> {
    let (score, matched) = fuzzy_match(name, needle)?;
    let score = score
        + if name.eq_ignore_ascii_case(needle) {
            COMPLETION_EXACT_MATCH_BONUS
        } else {
            0
        }
        + if first_char_matches(name, needle) {
            COMPLETION_FIRST_CHAR_BONUS
        } else {
            0
        };
    Some((score, matched))
}

/// The spelling in `names` that best matches `needle`, with its matched indices.
fn best_spelling<'a>(
    names: impl IntoIterator<Item = &'a str>,
    needle: &str,
) -> Option<(isize, &'a str, Vec<usize>)> {
    names
        .into_iter()
        .filter_map(|name| {
            let (score, matched) = name_match(name, needle)?;
            Some((score, name, matched))
        })
        .reduce(|best, candidate| {
            if candidate.0 > best.0 {
                candidate
            } else {
                best
            }
        })
}

/// The name of `entry` that best matches `needle`, with the matched char indices
/// and whether the localized display name was the best match.
pub(super) fn best_name_match<'a>(
    entry: &'a hactorio_data::Entry,
    needle: &str,
) -> Option<(isize, &'a str, Vec<usize>, bool)> {
    // A localized or modded display name can add words the ID lacks; a mere
    // reformatting of the ID cannot.
    let display_name = entry
        .display_name
        .as_deref()
        .filter(|name| !reforms_id(&entry.id, name));
    std::iter::once(entry.id.as_ref())
        .chain(display_name)
        .chain(aliases_for(&entry.id).iter().copied())
        .filter_map(|name| {
            let (score, matched) = name_match(name, needle)?;
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
    let after_equals = ["+=", "-=", "*=", "/="]
        .iter()
        .find_map(|op| rest.strip_prefix(op))
        .or_else(|| rest.strip_prefix('='))?;
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

const COMPLETION_KEYWORD_BONUS: f64 = 50.0;
/// Properties sort below the units a process already holds.
const COMPLETION_PROPERTY_BONUS: f64 = 40.0;
/// A unit already in the process net outranks the same name when it is absent.
const COMPLETION_PRESENT_ITEM_BONUS: f64 = 50.0;

/// Ranking bonus for a quality alias such as `quality-3`.
///
/// A quality has one glyph and several spellings, so the alias rows have to
/// outrank the near misses a short needle like `q1` also fuzzily matches.
const COMPLETION_ALIAS_BONUS: f64 = 120.0;

const KEYWORDS: &[&str] = &[
    "in", "into", "from", "making", "s", "second", "m", "minute", "h", "hour", "joule", "J", "kJ",
    "MJ", "GJ", "TJ", "watt", "W", "kW", "MW", "GW", "TW", "craft",
];

const PROPERTIES: &[&str] = &[
    "rate",
    "capacity",
    "power",
    "energy",
    "machines",
    "crafts",
    "productivity",
    "prod",
    "pollution",
    "abs",
    "in",
    "out",
    "net",
];

/// Signals that select like a property after a dot.
const SELECTOR_SIGNALS: [&str; 7] = [
    "stack-size",
    "speed",
    "time",
    "electric",
    "module-slots",
    "item",
    "fluid",
];

/// Stacked belt rows: one per tier, labelled by color but inserting the
/// `stacked-belt` prefix followed by the belt's own glyph.
const STACKED_BELTS: [(&str, &str); 4] = [
    ("stacked-yellow-belt", "transport-belt"),
    ("stacked-red-belt", "fast-transport-belt"),
    ("stacked-blue-belt", "express-transport-belt"),
    ("stacked-green-belt", "turbo-transport-belt"),
];

fn exact_rank(label: &str, insert: &str, needle: &str) -> u8 {
    let stripped = label.strip_prefix('.').unwrap_or(label);
    if label == needle || insert == needle || stripped == needle {
        2
    } else if label.eq_ignore_ascii_case(needle)
        || insert.eq_ignore_ascii_case(needle)
        || stripped.eq_ignore_ascii_case(needle)
    {
        1
    } else {
        0
    }
}

/// Every quality once, labelled by its best-matching spelling (§4.6).
///
/// A quality has one glyph and several spellings, so the aliases compete for a
/// single row like a prototype's names do: `q1`, `quality-1`, and `normal` all
/// complete the same inline quality glyph.
fn push_qualities(items: &mut Vec<(u8, f64, CompletionItem)>, needle: &str) {
    for quality in Quality::ALL {
        let glyph = quality.glyph().to_string();
        let (score, label, matched) = if needle.is_empty() {
            (0, quality.name(), Vec::new())
        } else {
            let Some(best) = best_spelling(quality.aliases().iter().copied(), needle) else {
                continue;
            };
            best
        };
        items.push((
            exact_rank(label, &glyph, needle),
            score as f64 + COMPLETION_ALIAS_BONUS,
            CompletionItem {
                insert: glyph.clone(),
                glyphs: glyph,
                label: label.to_string(),
                matched,
                name: None,
                name_matched: Vec::new(),
            },
        ));
    }
}

fn word_completion(
    label: &str,
    insert: String,
    glyphs: String,
    needle: &str,
    bonus: f64,
) -> Option<(u8, f64, CompletionItem)> {
    let (score, matched) = if needle.is_empty() {
        (0, Vec::new())
    } else {
        fuzzy_match(label, needle)?
    };
    Some((
        exact_rank(label, &insert, needle),
        score as f64 + bonus,
        CompletionItem {
            insert,
            glyphs,
            label: label.to_string(),
            matched,
            name: None,
            name_matched: Vec::new(),
        },
    ))
}

fn signal_matches(id: &str, needle: &str) -> bool {
    needle.is_empty()
        || id
            .to_ascii_lowercase()
            .starts_with(&needle.to_ascii_lowercase())
}

/// The right-hand side of `line`, when it is an assignment.
///
/// The selector after a dot binds to the operand it follows, so in
/// `p += p.rate` it reads `p`, not the process `p += p` would bind: the operand
/// value is the one before the write, and the statement's own value need not
/// even evaluate.
fn assignment_rhs<'a>(line: &'a str, registry: &Registry) -> Option<&'a str> {
    let span = definition_value_span(line, registry)?;
    line.get(span.start..span.end)
}

/// The value the selector after the dot ending at `word_start` applies to.
///
/// The text up to the dot is evaluated as its own document, so variables from
/// earlier lines and the rest of the current line are in scope. On an assignment
/// line the dot's operand is the right-hand side, so only that is evaluated.
/// `None` means the prefix does not evaluate, so nothing is known about the
/// value.
fn value_before_dot(
    text: &str,
    dot: usize,
    settings: &Settings,
    registry: &Registry,
) -> Option<Value> {
    let prefix: String = text.chars().take(dot).collect();
    let mut lines: Vec<&str> = prefix.split('\n').collect();
    if let Some(rhs) = lines.last().and_then(|line| assignment_rhs(line, registry)) {
        *lines.last_mut().unwrap() = rhs;
    }
    let evaluation = evaluate(&lines.join("\n"), registry, settings, &|| false);
    match evaluation.outputs.last()? {
        Output::Value(value) => Some((**value).clone()),
        _ => None,
    }
}

/// Whether `entry` is a unit of the process `value` holds.
///
/// Unit selectors only read a process; an item outside its net would select
/// nothing.
fn unit_in_value(value: &Value, id: EntryId, entry: &Entry) -> bool {
    let Value::Process(process) = value else {
        return false;
    };
    let resource = if entry.fluid {
        Resource::Fluid(id)
    } else {
        Resource::Item(Item {
            id,
            quality: Quality::Normal,
        })
    };
    process_resources(process).contains(&resource)
}

pub(super) fn completion_items(
    registry: &Registry,
    settings: &Settings,
    text: &str,
    word_start: usize,
    word: &str,
) -> Vec<CompletionItem> {
    let before: Vec<char> = text.chars().take(word_start).collect();
    // A dot may be separated from what follows it by spaces, so the selector
    // context is the last non-space character, not the last character.
    let dot = before
        .iter()
        .rev()
        .take_while(|ch| **ch != '\n')
        .position(|ch| !ch.is_whitespace())
        .map(|offset| before.len() - 1 - offset)
        .filter(|index| before[*index] == '.');
    let context = if dot.is_some() {
        CompletionContext::Property
    } else {
        match previous_word(&before).as_str() {
            "making" => CompletionContext::Recipe,
            "with" => CompletionContext::Config,
            "in" => CompletionContext::Machine,
            _ => CompletionContext::Normal,
        }
    };
    // Properties and unit selectors only exist for a value; resolve it once.
    let value = match dot {
        Some(dot) => value_before_dot(text, dot, settings, registry),
        None => None,
    };
    let needle = word.trim_start_matches(':').trim_end_matches(':');
    // An empty needle lists everything the context allows; the editor only asks
    // for it when the popup is opened explicitly (Ctrl+Space) or after a dot.
    if needle.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        return Vec::new();
    }

    let mut items: Vec<(u8, f64, CompletionItem)> = registry
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, entry)| match context {
            // A hidden item can share its entry with a visible recipe (the
            // rocket part), and a visible item can share it with a hidden
            // recipe (the pistol): recipe offers depend on the recipe alone.
            CompletionContext::Recipe => entry
                .recipe
                .is_some_and(|recipe| !registry.recipe(recipe).hidden),
            _ if entry.hidden => false,
            CompletionContext::Machine => entry.machine().is_some(),
            CompletionContext::Config => entry.module.is_some() || entry.beacon.is_some(),
            CompletionContext::Property => match &value {
                // A process and a quantity both select by resource, so
                // every item and fluid is a selector after their dot.
                Some(Value::Process(_)) => entry.is_item_or_fluid(),
                Some(Value::Multi(multi)) => multi.quantity.is_some() && entry.is_item_or_fluid(),
                Some(Value::Quality(_)) => false,
                None => entry.is_item_or_fluid(),
            },
            CompletionContext::Normal => true,
        })
        .filter_map(|(index, entry)| {
            let (score, matched_name, matched, display_matched) = if needle.is_empty() {
                (0, entry.id.as_ref(), Vec::new(), false)
            } else {
                best_name_match(entry, needle)?
            };
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
            let insert = entry.glyph.to_string();
            let rank = exact_rank(label, &insert, needle);
            let present = value
                .as_ref()
                .is_some_and(|value| unit_in_value(value, EntryId::from_index(index), entry));
            let mut score = if needle.is_empty() {
                score as f64
            } else {
                completion_rank(score, references)
            };
            if present {
                score += COMPLETION_PRESENT_ITEM_BONUS;
            }
            Some((
                rank,
                score,
                CompletionItem {
                    insert,
                    glyphs: entry.glyph.to_string(),
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
            let (score, matched) = if needle.is_empty() {
                (0, Vec::new())
            } else {
                let Some(matched) = fuzzy_match(&name, needle) else {
                    continue;
                };
                matched
            };
            items.push((
                exact_rank(&name, &name, needle),
                score as f64 + COMPLETION_VARIABLE_BONUS,
                CompletionItem {
                    insert: name.clone(),
                    glyphs: crate::glyphs::VARIABLE.to_string(),
                    label: name,
                    matched,
                    name: None,
                    name_matched: Vec::new(),
                },
            ));
        }

        for keyword in KEYWORDS {
            if let Some(item) = word_completion(
                keyword,
                (*keyword).to_string(),
                EMPTY_GLYPH.to_string(),
                needle,
                COMPLETION_KEYWORD_BONUS,
            ) {
                items.push(item);
            }
        }
    }

    if matches!(
        context,
        CompletionContext::Normal | CompletionContext::Recipe
    ) {
        // A quality qualifies a prototype, filters a quantity, and can precede
        // a recipe after `making`, so every quality is offered (§4.6). Its
        // spellings all match the one glyph row.
        push_qualities(&mut items, needle);
    }

    if context == CompletionContext::Property {
        // A selector names a resource, and a resource can be qualified, so the
        // qualities belong here too: `p.:rare::iron-plate:`.
        let qualified = value.as_ref().is_some_and(|value| match value {
            Value::Process(_) => true,
            Value::Multi(multi) => multi.quantity.is_some(),
            Value::Quality(_) => false,
        });
        if qualified {
            push_qualities(&mut items, needle);
        }
        for property in PROPERTIES {
            if let Some(value) = &value
                && !property_is_available(value, property, registry, settings)
            {
                continue;
            }
            let label = format!(".{property}");
            if let Some(item) = word_completion(
                &label,
                (*property).to_string(),
                EMPTY_GLYPH.to_string(),
                needle,
                COMPLETION_PROPERTY_BONUS,
            ) {
                items.push(item);
            }
        }
        for id in SELECTOR_SIGNALS {
            if let Some(value) = &value
                && !property_is_available(value, id, registry, settings)
            {
                continue;
            }
            let Some(signal) = SIGNALS.iter().find(|signal| signal.id == id) else {
                continue;
            };
            // A typed prefix keeps the signal's dominance; a loose fuzzy match
            // (`p.ectric`) is discovered but ranks with the properties.
            let bonus = COMPLETION_PROPERTY_BONUS;
            let label = format!(".{}", signal.id);
            if let Some(item) = word_completion(
                &label,
                signal.glyph.to_string(),
                signal.glyph.to_string(),
                needle,
                bonus,
            ) {
                items.push(item);
            }
        }
    } else if context == CompletionContext::Normal {
        for signal in SIGNALS.iter().filter(|signal| {
            matches!(signal.id, "stack-size" | "stacked-belt" | "recycling")
                && signal_matches(signal.id, needle)
        }) {
            let label = signal.id.to_string();
            if let Some(item) = word_completion(
                &label,
                signal.glyph.to_string(),
                signal.glyph.to_string(),
                needle,
                COMPLETION_PROPERTY_BONUS,
            ) {
                items.push(item);
            }
        }
        if let Some(stacked) = SIGNALS.iter().find(|signal| signal.id == "stacked-belt") {
            for (label, belt_id) in STACKED_BELTS {
                let Some(belt) = registry.entry(belt_id) else {
                    continue;
                };
                let glyphs = format!("{}{}", stacked.glyph, belt.glyph);
                if let Some(item) = word_completion(label, glyphs.clone(), glyphs, needle, 0.0) {
                    items.push(item);
                }
            }
        }
    }

    // An empty needle scores every name the same, so falling back to the
    // shortest label would just list short names in a meaningless order. List
    // them alphabetically instead.
    items.sort_by(|(a_exact, a_rank, a), (b_exact, b_rank, b)| {
        b_exact
            .cmp(a_exact)
            .then_with(|| b_rank.total_cmp(a_rank))
            .then_with(|| {
                if needle.is_empty() {
                    a.label.cmp(&b.label)
                } else {
                    a.label
                        .len()
                        .cmp(&b.label.len())
                        .then_with(|| a.label.cmp(&b.label))
                }
            })
    });
    items.into_iter().map(|(_, _, item)| item).collect()
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

/// Appends the glyph column: the row's glyph and its separating space.
fn append_glyph_column(
    job: &mut egui::text::LayoutJob,
    item: &CompletionItem,
    normal: &egui::TextFormat,
) {
    // A stacked belt row shows two glyphs, so its label starts one cell later
    // than the others. That is accepted rather than padding every row.
    job.append(&item.glyphs, 0.0, normal.clone());
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
    append_glyph_column(&mut job, item, &normal);
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

/// The popup's measured width, cached until the items or font change.
#[derive(Clone)]
struct PopupMetrics {
    font_id: egui::FontId,
    /// Character count of the widest label, for column alignment.
    label_width: usize,
    /// Pixel width of the widest row.
    text_width: f32,
}

/// Measures the widest completion row without laying out text.
///
/// A glyph advances exactly its font size, and monospace text advances by its
/// character count. A non-ASCII display name is summed per character, because
/// wide scripts do not share the ASCII advance.
fn popup_metrics(ui: &Ui, items: &[CompletionItem], font_id: &egui::FontId) -> PopupMetrics {
    let label_width = items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0);
    let advance = ui.fonts_mut(|fonts| fonts.glyph_width(font_id, 'M'));
    let mut wide: HashMap<char, f32> = HashMap::new();
    let mut text_width: f32 = 0.0;
    for item in items {
        let mut width = item.glyphs.chars().count() as f32 * font_id.size + advance;
        width += advance_of(ui, font_id, &item.label, advance, &mut wide);
        if let Some(name) = &item.name {
            let padding =
                label_width.saturating_sub(item.label.chars().count()) + COMPLETION_NAME_GAP;
            width += padding as f32 * advance;
            width += advance_of(ui, font_id, name, advance, &mut wide);
        }
        text_width = text_width.max(width);
    }
    PopupMetrics {
        font_id: font_id.clone(),
        label_width,
        text_width,
    }
}

/// The width of `text`, whose characters are monospace widths unless they fall
/// outside ASCII.
fn advance_of(
    ui: &Ui,
    font_id: &egui::FontId,
    text: &str,
    advance: f32,
    wide: &mut HashMap<char, f32>,
) -> f32 {
    if text.is_ascii() {
        return text.len() as f32 * advance;
    }
    text.chars()
        .map(|ch| {
            *wide
                .entry(ch)
                .or_insert_with(|| ui.fonts_mut(|fonts| fonts.glyph_width(font_id, ch)))
        })
        .sum()
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
    /// Replaces the candidate rows, dropping the measured popup width.
    pub(super) fn set_items(&mut self, items: Vec<CompletionItem>) {
        self.items = items;
        self.metrics = None;
    }

    /// Whether the popup should be shown, unless it was dismissed for this word.
    pub(super) fn is_open(&self) -> bool {
        !self.items.is_empty() && self.dismissed.as_deref() != Some(self.word.as_str())
    }
}

/// What the editor hands to the popup for one frame.
pub(super) struct PopupRequest<'a> {
    pub active_tab: usize,
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
        active_tab,
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
    egui::Area::new(egui::Id::unique(("completion", active_tab)))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            let style = ui.style();
            let font_id = TextStyle::Monospace.resolve(style);
            let cached = completion
                .metrics
                .as_ref()
                .filter(|metrics| metrics.font_id == font_id)
                .cloned();
            let metrics = match cached {
                Some(metrics) => metrics,
                None => {
                    let metrics = popup_metrics(ui, &completion.items, &font_id);
                    completion.metrics = Some(metrics.clone());
                    metrics
                }
            };
            let label_width = metrics.label_width;
            let text_width = metrics.text_width;
            let chrome = egui::Frame::popup(style);
            let chrome_width = chrome.inner_margin.sum().x + 2.0 * chrome.stroke.width;
            let content_width = (text_width
                + 2.0 * style.spacing.button_padding.x
                + style.spacing.scroll.allocated_width())
            .max(300.0);
            let total_width =
                (content_width + chrome_width).min(ui.ctx().viewport_rect().width() - 16.0);
            let content_width = (total_width - chrome_width).max(100.0);
            // One row is a single-line button: its text, plus the button padding.
            let text_height = ui.fonts_mut(|fonts| fonts.row_height(&font_id));
            let row_height = (text_height + 2.0 * ui.spacing().button_padding.y)
                .max(ui.spacing().interact_size.y);

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
                    .id_salt(("completion_scroll", active_tab))
                    .max_height(COMPLETION_MAX_HEIGHT)
                    .show_rows(ui, row_height, completion.items.len(), |ui, range| {
                        let row_width = ui.available_width();
                        let range_start = range.start;
                        let mut first_row: Option<egui::Rect> = None;
                        let mut selected_row: Option<egui::Rect> = None;
                        for index in range {
                            let item = &completion.items[index];
                            let job = completion_layout_job(
                                item,
                                label_width,
                                index == selected,
                                &font_id,
                                ui.visuals(),
                            );
                            // The full popup width is clickable, not just the text,
                            // and every row occupies the pitch `show_rows` assumes.
                            let response = ui
                                .allocate_ui_with_layout(
                                    egui::vec2(row_width, row_height),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.add(
                                            egui::Button::selectable(index == selected, job)
                                                .min_size(egui::vec2(row_width, row_height)),
                                        )
                                    },
                                )
                                .inner;
                            first_row.get_or_insert(response.rect);
                            if index == selected {
                                selected_row = Some(response.rect);
                                #[cfg(test)]
                                {
                                    completion.selected_rect = Some(response.rect);
                                }
                            }
                            if response.clicked() {
                                clicked = Some(index);
                            }
                        }
                        if needs_scroll {
                            let step = row_height + ui.spacing().item_spacing.y;
                            let target = selected_row.or_else(|| {
                                let first = first_row?;
                                let top =
                                    first.top() + (selected as f32 - range_start as f32) * step;
                                Some(egui::Rect::from_min_size(
                                    egui::pos2(first.left(), top),
                                    egui::vec2(first.width(), row_height),
                                ))
                            });
                            if let Some(target) = target {
                                ui.scroll_to_rect_animation(
                                    target,
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
