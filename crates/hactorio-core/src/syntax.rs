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
    /// A numeric literal, including its `k`/`M`/`G`/`T` multiplier.
    Number,
    /// From `#` to the end of the line.
    Comment,
    /// A closed, non-empty `:id:` token shown as text rather than a glyph.
    Icon,
}

/// The highlighted byte ranges of `text`, in order and non-overlapping.
///
/// Boundaries follow the lexer's tokens: a comment runs to the next newline, a
/// number includes one optional `.` fraction and one optional scale letter, and
/// a word is classified only when [`keyword`] recognizes it.
pub fn highlights(text: &str) -> Vec<(Span, Highlight)> {
    let mut spans = Vec::new();
    let mut index = 0;
    while index < text.len() {
        match next_char(text, index) {
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
                    index = end;
                }
                _ => index += 1,
            },
            '0'..='9' => index = number_end(text, index, &mut spans),
            '.' if next_char(text, index + 1).is_ascii_digit() => {
                index = number_end(text, index, &mut spans)
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = index;
                while {
                    let ch = next_char(text, index);
                    ch.is_ascii_alphanumeric() || ch == '_'
                } {
                    index += 1;
                }
                let highlight = match keyword(&text[start..index]) {
                    Some(Keyword::In | Keyword::With | Keyword::Making) => {
                        Some(Highlight::OperatorKeyword)
                    }
                    Some(
                        Keyword::Joule(_) | Keyword::Watt(_) | Keyword::Craft | Keyword::Time(_),
                    ) => Some(Highlight::UnitKeyword),
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
    if matches!(next_char(text, index), 'k' | 'M' | 'G' | 'T') {
        index += 1;
    }
    spans.push((Span::new(start as u32, index as u32), Highlight::Number));
    index
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
    fn scales_stay_with_numbers_and_units_stay_separate() {
        assert_eq!(
            classified("1k + 2MJ + 4 J + 5 minute + 6 craft"),
            vec![
                ("1k", Highlight::Number),
                ("2M", Highlight::Number),
                ("J", Highlight::UnitKeyword),
                ("4", Highlight::Number),
                ("J", Highlight::UnitKeyword),
                ("5", Highlight::Number),
                ("minute", Highlight::UnitKeyword),
                ("6", Highlight::Number),
                ("craft", Highlight::UnitKeyword),
            ]
        );
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
    fn signals_are_icons() {
        assert_eq!(
            classified("2 :stack-size:"),
            vec![("2", Highlight::Number), (":stack-size:", Highlight::Icon)]
        );
    }
}
