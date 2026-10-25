//! Shared syntax primitives, including the keyword table and the editor's
//! syntax-highlighting scan.

use crate::settings::TimeUnit;

/// A byte range within one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// A reserved word and the value it denotes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Keyword {
    In,
    With,
    Making,
    Joule(f64),
    Watt(f64),
    Craft,
    Time(TimeUnit),
}

/// The keyword `word` spells, if any.
pub(crate) fn keyword(word: &str) -> Option<Keyword> {
    Some(match word {
        "in" => Keyword::In,
        "with" => Keyword::With,
        "making" => Keyword::Making,
        "joule" | "J" => Keyword::Joule(1.0),
        "kJ" => Keyword::Joule(1e3),
        "MJ" => Keyword::Joule(1e6),
        "GJ" => Keyword::Joule(1e9),
        "TJ" => Keyword::Joule(1e12),
        "watt" | "W" => Keyword::Watt(1.0),
        "kW" => Keyword::Watt(1e3),
        "MW" => Keyword::Watt(1e6),
        "GW" => Keyword::Watt(1e9),
        "TW" => Keyword::Watt(1e12),
        "craft" => Keyword::Craft,
        _ => Keyword::Time(TimeUnit::from_word(word)?),
    })
}

/// A category of text the editor paints in its own color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Highlight {
    /// An operator keyword: `in`, `with`, `making`.
    OperatorKeyword,
    /// A unit keyword: `s`, `second`, `J`, `kJ`, `craft`, ...
    UnitKeyword,
    /// A numeric literal, without any trailing multiplier or unit.
    Number,
    /// A variable's name, where it is defined and where it is used.
    Variable,
    /// From `#` to the end of the line.
    Comment,
    /// A closed, non-empty `:id:` token shown as text rather than a glyph.
    Icon,
}

/// The highlighted byte ranges of `text`, in order and non-overlapping.
///
/// Boundaries follow the lexer's tokens: a comment runs to the next newline, a
/// number includes one optional `.` fraction, and a word is classified when
/// [`keyword`] recognizes it, when it names a variable, or when it is a bare
/// scale letter directly after a number. A scale letter is part of the unit
/// keyword that follows it, so `1kJ` is the number `1` and the unit keyword
/// `kJ`.
///
/// A word names a variable while it is the name `=` binds or any identifier
/// after that binding, so uses are colored only from the line that binds the
/// name onward. A compound assignment such as `+=` only uses a name, and an
/// identifier after `.` is a property, never a variable.
pub fn highlights(text: &str) -> Vec<(Span, Highlight)> {
    let mut spans = Vec::new();
    let mut variables = std::collections::HashSet::new();
    let mut statement_start = true;
    let mut index = 0;
    while index < text.len() {
        match next_char(text, index) {
            '\n' => {
                statement_start = true;
                index += 1;
            }
            '#' => {
                let end = text[index..]
                    .find('\n')
                    .map_or(text.len(), |offset| index + offset);
                spans.push((Span::new(index as u32, end as u32), Highlight::Comment));
                index = end;
            }
            ':' => match text[index + 1..].find([':', '\n']) {
                Some(offset) if offset > 0 && text.as_bytes()[index + 1 + offset] == b':' => {
                    let end = index + 1 + offset + 1;
                    spans.push((Span::new(index as u32, end as u32), Highlight::Icon));
                    statement_start = false;
                    index = end;
                }
                _ => index += 1,
            },
            '0'..='9' => {
                index = number_end(text, index, &mut spans);
                statement_start = false;
            }
            '.' if next_char(text, index + 1).is_ascii_digit() => {
                index = number_end(text, index, &mut spans);
                statement_start = false;
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = index;
                while {
                    let ch = next_char(text, index);
                    ch.is_ascii_alphanumeric() || ch == '_'
                } {
                    index += 1;
                }
                let word = &text[start..index];
                let after_dot = preceded_by_dot(text, start);
                let defines = statement_start && assignment_follows(text, index);
                statement_start = false;
                let highlight = match keyword(word) {
                    Some(Keyword::In | Keyword::With | Keyword::Making) => {
                        Some(Highlight::OperatorKeyword)
                    }
                    Some(
                        Keyword::Joule(_) | Keyword::Watt(_) | Keyword::Craft | Keyword::Time(_),
                    ) => Some(Highlight::UnitKeyword),
                    // A bare multiplier only means a scale when it directly
                    // follows a number; elsewhere it is an ordinary word.
                    None if is_scale(word)
                        && matches!(
                            spans.last(),
                            Some((span, Highlight::Number)) if span.end as usize == start
                        ) =>
                    {
                        Some(Highlight::UnitKeyword)
                    }
                    None if defines => {
                        variables.insert(word);
                        Some(Highlight::Variable)
                    }
                    // `value.rate` selects the property `rate`; a variable of
                    // that name is not in play there.
                    None if after_dot => None,
                    None if variables.contains(word) => Some(Highlight::Variable),
                    None => None,
                };
                if let Some(highlight) = highlight {
                    spans.push((Span::new(start as u32, index as u32), highlight));
                }
            }
            _ => index += next_char(text, index).len_utf8(),
        }
    }
    spans
}

/// Consumes one numeric literal from `index`, pushes its span, returns the end.
fn number_end(text: &str, mut index: usize, spans: &mut Vec<(Span, Highlight)>) -> usize {
    let start = index;
    while next_char(text, index).is_ascii_digit() {
        index += 1;
    }
    if next_char(text, index) == '.' {
        index += 1;
        while next_char(text, index).is_ascii_digit() {
            index += 1;
        }
    }
    spans.push((Span::new(start as u32, index as u32), Highlight::Number));
    index
}

/// Whether only whitespace separates the word at `start` from a `.`.
fn preceded_by_dot(text: &str, mut start: usize) -> bool {
    while start > 0 && matches!(text.as_bytes()[start - 1], b' ' | b'\t' | b'\r') {
        start -= 1;
    }
    start > 0 && text.as_bytes()[start - 1] == b'.'
}

/// Whether `=` follows the identifier ending at `index`.
///
/// Only `=` binds a name; a compound assignment like `+=` needs a name that is
/// already defined.
fn assignment_follows(text: &str, mut index: usize) -> bool {
    while matches!(next_char(text, index), ' ' | '\t' | '\r') {
        index += 1;
    }
    next_char(text, index) == '='
}

/// Whether the word ending at `index` is a bare scale letter a number may be
/// followed by.
fn is_scale(word: &str) -> bool {
    matches!(word, "k" | "M" | "G" | "T")
}

fn next_char(text: &str, index: usize) -> char {
    text[index..].chars().next().unwrap_or('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classified(text: &str) -> Vec<(&str, Highlight)> {
        highlights(text)
            .into_iter()
            .map(|(span, highlight)| (&text[span.start as usize..span.end as usize], highlight))
            .collect()
    }

    #[test]
    fn scans_every_category_in_source_order() {
        assert_eq!(
            classified("3 in :assembling-machine-3: 2s # note"),
            vec![
                ("3", Highlight::Number),
                ("in", Highlight::OperatorKeyword),
                (":assembling-machine-3:", Highlight::Icon),
                ("2", Highlight::Number),
                ("s", Highlight::UnitKeyword),
                ("# note", Highlight::Comment),
            ]
        );
    }

    #[test]
    fn scale_letters_belong_to_the_unit_keyword() {
        assert_eq!(
            classified("1k + 2MJ + 4 J + 5 minute + 6 craft + 7k J"),
            vec![
                ("1", Highlight::Number),
                ("k", Highlight::UnitKeyword),
                ("2", Highlight::Number),
                ("MJ", Highlight::UnitKeyword),
                ("4", Highlight::Number),
                ("J", Highlight::UnitKeyword),
                ("5", Highlight::Number),
                ("minute", Highlight::UnitKeyword),
                ("6", Highlight::Number),
                ("craft", Highlight::UnitKeyword),
                ("7", Highlight::Number),
                ("k", Highlight::UnitKeyword),
                ("J", Highlight::UnitKeyword),
            ]
        );
    }

    #[test]
    fn scale_letters_away_from_a_number_are_not_units() {
        assert!(classified("k M G T").is_empty());
        assert_eq!(classified("1 M"), vec![("1", Highlight::Number)],);
    }

    #[test]
    fn comments_swallow_everything_after_the_hash() {
        assert_eq!(
            classified("# 3 in :x: 2s"),
            vec![("# 3 in :x: 2s", Highlight::Comment)]
        );
        assert_eq!(classified("#"), vec![("#", Highlight::Comment)]);
    }

    #[test]
    fn icon_tokens_do_not_pair_across_lines() {
        assert_eq!(
            classified(":iron-plate\n:iron-plate:"),
            vec![(":iron-plate:", Highlight::Icon)]
        );
    }

    #[test]
    fn empty_and_unterminated_icon_tokens_are_not_icons() {
        assert!(classified("::").is_empty());
        assert!(classified(":iron-plate").is_empty());
    }

    #[test]
    fn keywords_must_be_whole_words() {
        assert!(classified("withholding").is_empty());
        assert_eq!(
            classified("with"),
            vec![("with", Highlight::OperatorKeyword)]
        );
    }

    #[test]
    fn leading_dot_numbers_are_numbers() {
        assert_eq!(
            classified(".5 MJ"),
            vec![(".5", Highlight::Number), ("MJ", Highlight::UnitKeyword)]
        );
    }

    #[test]
    fn variable_definitions_and_uses_are_variables() {
        assert_eq!(
            classified("plates = 2 in :assembling-machine-3:\nplates * 3\nplates += 1"),
            vec![
                ("plates", Highlight::Variable),
                ("2", Highlight::Number),
                ("in", Highlight::OperatorKeyword),
                (":assembling-machine-3:", Highlight::Icon),
                ("plates", Highlight::Variable),
                ("3", Highlight::Number),
                ("plates", Highlight::Variable),
                ("1", Highlight::Number),
            ]
        );
    }

    #[test]
    fn only_equals_at_the_start_of_a_line_binds_a_name() {
        assert_eq!(
            classified("1 + count = 2"),
            vec![("1", Highlight::Number), ("2", Highlight::Number)]
        );
        assert!(classified("count").is_empty());
        assert_eq!(
            classified("count = 2\ncount"),
            vec![
                ("count", Highlight::Variable),
                ("2", Highlight::Number),
                ("count", Highlight::Variable)
            ]
        );
    }

    #[test]
    fn compound_assignment_only_uses_a_name() {
        assert_eq!(
            classified("count += 1\ncount"),
            vec![("1", Highlight::Number)]
        );
        assert_eq!(
            classified("count = 1\ncount += 1\ncount"),
            vec![
                ("count", Highlight::Variable),
                ("1", Highlight::Number),
                ("count", Highlight::Variable),
                ("1", Highlight::Number),
                ("count", Highlight::Variable),
            ]
        );
    }

    #[test]
    fn keywords_and_properties_win_over_variable_names() {
        assert_eq!(
            classified("rate = 2\nrate.rate\n.rate"),
            vec![
                ("rate", Highlight::Variable),
                ("2", Highlight::Number),
                ("rate", Highlight::Variable),
            ]
        );
        assert_eq!(
            classified("in = 2\nin"),
            vec![
                ("in", Highlight::OperatorKeyword),
                ("2", Highlight::Number),
                ("in", Highlight::OperatorKeyword)
            ]
        );
        assert_eq!(
            classified("J = 2"),
            vec![("J", Highlight::UnitKeyword), ("2", Highlight::Number)]
        );
    }

    #[test]
    fn signals_are_icons() {
        assert_eq!(
            classified("2 :stack-size:"),
            vec![("2", Highlight::Number), (":stack-size:", Highlight::Icon)]
        );
    }
}
