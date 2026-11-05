//! The tokenizer (§4.4).
//!
//! Lexing is per line, so a quality cannot qualify a prototype on the next
//! line (§4.5).

use hactorio_data::{EntryId, Registry, signal_by_glyph};

use super::parse::{ParseError, Span};
use super::value::Quality;

/// A fixed signal, which is not a registry entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    StackSize,
    StackedBelt,
    Time,
    Speed,
    Electric,
    ModuleSlots,
    Item,
    Fluid,
    Recycling,
}

impl Signal {
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "stack-size" => Signal::StackSize,
            "stacked-belt" => Signal::StackedBelt,
            "time" => Signal::Time,
            "speed" => Signal::Speed,
            "electric" => Signal::Electric,
            "module-slots" => Signal::ModuleSlots,
            "item" => Signal::Item,
            "fluid" => Signal::Fluid,
            "recycling" => Signal::Recycling,
            _ => return None,
        })
    }

    pub fn from_glyph(glyph: char) -> Option<Self> {
        Signal::from_id(signal_by_glyph(glyph)?.id)
    }

    pub fn id(self) -> &'static str {
        match self {
            Signal::StackSize => "stack-size",
            Signal::StackedBelt => "stacked-belt",
            Signal::Time => "time",
            Signal::Speed => "speed",
            Signal::Electric => "electric",
            Signal::ModuleSlots => "module-slots",
            Signal::Item => "item",
            Signal::Fluid => "fluid",
            Signal::Recycling => "recycling",
        }
    }

    /// Signals that may begin an operand.
    pub fn starts_operand(self) -> bool {
        matches!(
            self,
            Signal::StackSize | Signal::StackedBelt | Signal::Recycling
        )
    }
}

/// A unit keyword, carrying its magnitude (§4.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unit {
    Second(f64),
    Joule(f64),
    Watt(f64),
    Craft(f64),
}

impl Unit {
    fn from_keyword(word: &str) -> Option<Self> {
        Some(match word {
            "second" | "s" => Unit::Second(1.0),
            "minute" | "m" => Unit::Second(60.0),
            "hour" | "h" => Unit::Second(3600.0),
            "joule" | "J" => Unit::Joule(1.0),
            "kJ" => Unit::Joule(1e3),
            "MJ" => Unit::Joule(1e6),
            "GJ" => Unit::Joule(1e9),
            "TJ" => Unit::Joule(1e12),
            "watt" | "W" => Unit::Watt(1.0),
            "kW" => Unit::Watt(1e3),
            "MW" => Unit::Watt(1e6),
            "GW" => Unit::Watt(1e9),
            "TW" => Unit::Watt(1e12),
            "craft" => Unit::Craft(1.0),
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Making,
    In,
    Into,
    From,
    Loop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Unit(Unit),
    Prototype(EntryId),
    Qualified { id: EntryId, quality: Quality },
    Quality(Quality),
    Signal(Signal),
    Ident(String),
    Keyword(Keyword),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    EqualEqual,
    NotEqual,
    Dot,
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
}

impl Token {
    /// Whether this token can start an operand, and therefore continue a
    /// juxtaposition chain. Unary minus is excluded: `-` in that position is
    /// the binary operator.
    pub fn starts_operand(&self) -> bool {
        match self {
            Token::Number(_)
            | Token::Unit(_)
            | Token::Prototype(_)
            | Token::Qualified { .. }
            | Token::Quality(_)
            | Token::Ident(_)
            | Token::LParen => true,
            Token::Signal(signal) => signal.starts_operand(),
            _ => false,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Token::Number(value) => format!("`{value}`"),
            Token::Unit(_) => "a unit".to_string(),
            Token::Prototype(_) | Token::Qualified { .. } => "a prototype".to_string(),
            Token::Quality(quality) => format!("`:{}:`", quality.name()),
            Token::Signal(signal) => format!("`:{}:`", signal.id()),
            Token::Ident(name) => format!("`{name}`"),
            Token::Keyword(keyword) => format!("`{keyword:?}`").to_lowercase(),
            Token::LParen => "`(`".to_string(),
            Token::RParen => "`)`".to_string(),
            Token::LBrace => "`{`".to_string(),
            Token::RBrace => "`}`".to_string(),
            other => format!("`{}`", other.symbol()),
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Star => "*",
            Token::Slash => "/",
            Token::Caret => "^",
            Token::Less => "<",
            Token::Greater => ">",
            Token::LessEqual => "<=",
            Token::GreaterEqual => ">=",
            Token::EqualEqual => "==",
            Token::NotEqual => "!=",
            Token::Dot => ".",
            Token::Assign => "=",
            Token::AddAssign => "+=",
            Token::SubAssign => "-=",
            Token::MulAssign => "*=",
            Token::DivAssign => "/=",
            _ => "",
        }
    }
}

pub fn lex(line: &str, registry: &Registry) -> Result<Vec<(Token, Span)>, ParseError> {
    let mut tokens = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let ch = line[index..].chars().next().unwrap();
        if ch.is_whitespace() {
            index += ch.len_utf8();
            continue;
        }
        if ch == '#' {
            break;
        }
        if ch.is_ascii_digit() || (ch == '.' && starts_number(&line[index..])) {
            let start = index;
            let (mut value, mut end) = scan_number(line, index)?;
            let run_end = scan_identifier(line, end);
            if run_end > end {
                let run = &line[end..run_end];
                if let Some(scale) = scale_suffix(run) {
                    value *= scale;
                    end = run_end;
                } else if let Some(unit) = Unit::from_keyword(run) {
                    tokens.push((Token::Number(value), Span::new(start, end)));
                    tokens.push((Token::Unit(unit), Span::new(end, run_end)));
                    index = run_end;
                    continue;
                } else {
                    return Err(ParseError::new(
                        Span::new(end, run_end),
                        format!("`{run}` cannot follow a number; write a space"),
                    ));
                }
            }
            tokens.push((Token::Number(value), Span::new(start, end)));
            index = end.max(start + 1);
            continue;
        }
        if ch == ':' {
            let start = index;
            let (id, end) = scan_token_body(line, index)?;
            index = end;
            match resolve(&id, registry) {
                Ok(token) => tokens.push((token, Span::new(start, end))),
                Err(message) => return Err(ParseError::new(Span::new(start, end), message)),
            }
            continue;
        }
        if let Some(quality) = hactorio_data::quality_by_glyph(ch) {
            let start = index;
            tokens.push((
                Token::Quality(
                    Quality::from_name(quality.id).expect("a quality glyph names a quality"),
                ),
                Span::new(start, start + ch.len_utf8()),
            ));
            index += ch.len_utf8();
            continue;
        }
        if let Some(signal) = Signal::from_glyph(ch) {
            let start = index;
            tokens.push((
                Token::Signal(signal),
                Span::new(start, start + ch.len_utf8()),
            ));
            index += ch.len_utf8();
            continue;
        }
        if let Some(entry) = registry.entry_by_glyph(ch) {
            let id = registry
                .entry_id(entry.id.as_ref())
                .expect("a glyph belongs to a known entry");
            let start = index;
            tokens.push((
                Token::Prototype(id),
                Span::new(start, start + ch.len_utf8()),
            ));
            index += ch.len_utf8();
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            let end = scan_identifier(line, index);
            let word = &line[index..end];
            let token = match keyword(word) {
                Some(keyword) => Token::Keyword(keyword),
                None => match Unit::from_keyword(word) {
                    Some(unit) => Token::Unit(unit),
                    None => Token::Ident(word.to_string()),
                },
            };
            tokens.push((token, Span::new(start, end)));
            index = end;
            continue;
        }

        let start = index;
        let (token, width) = match ch {
            '(' => (Token::LParen, 1),
            ')' => (Token::RParen, 1),
            '{' => (Token::LBrace, 1),
            '}' => (Token::RBrace, 1),
            '+' => match line[index + 1..].chars().next() {
                Some('=') => (Token::AddAssign, 2),
                _ => (Token::Plus, 1),
            },
            '-' => match line[index + 1..].chars().next() {
                Some('=') => (Token::SubAssign, 2),
                _ => (Token::Minus, 1),
            },
            '*' => match line[index + 1..].chars().next() {
                Some('=') => (Token::MulAssign, 2),
                _ => (Token::Star, 1),
            },
            '/' => match line[index + 1..].chars().next() {
                Some('=') => (Token::DivAssign, 2),
                _ => (Token::Slash, 1),
            },
            '^' => (Token::Caret, 1),
            '<' => match line[index + 1..].chars().next() {
                Some('=') => (Token::LessEqual, 2),
                _ => (Token::Less, 1),
            },
            '>' => match line[index + 1..].chars().next() {
                Some('=') => (Token::GreaterEqual, 2),
                _ => (Token::Greater, 1),
            },
            '=' => match line[index + 1..].chars().next() {
                Some('=') => (Token::EqualEqual, 2),
                _ => (Token::Assign, 1),
            },
            '!' => match line[index + 1..].chars().next() {
                Some('=') => (Token::NotEqual, 2),
                _ => {
                    return Err(ParseError::new(
                        Span::new(start, start + 1),
                        "expected `!=`",
                    ));
                }
            },
            '.' => (Token::Dot, 1),
            other => {
                return Err(ParseError::new(
                    Span::new(start, start + other.len_utf8()),
                    format!("unexpected `{other}`"),
                ));
            }
        };
        tokens.push((token, Span::new(start, index + width)));
        index += width;
    }

    Ok(merge_qualified(tokens))
}

/// A quality token directly before a prototype token is one token (§4.5).
fn merge_qualified(tokens: Vec<(Token, Span)>) -> Vec<(Token, Span)> {
    let mut out: Vec<(Token, Span)> = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();
    while let Some((token, span)) = iter.next() {
        if let Token::Quality(quality) = token {
            if let Some((Token::Prototype(id), next)) = iter.peek() {
                let merged = Span::new(span.start, next.end);
                out.push((Token::Qualified { id: *id, quality }, merged));
                iter.next();
                continue;
            }
            out.push((Token::Quality(quality), span));
            continue;
        }
        out.push((token, span));
    }
    out
}

fn resolve(id: &str, registry: &Registry) -> Result<Token, String> {
    if let Some(signal) = Signal::from_id(id) {
        return Ok(Token::Signal(signal));
    }
    if let Some(quality) = Quality::from_name(id) {
        return Ok(Token::Quality(quality));
    }
    match registry.entry_id(id) {
        Some(entry) => Ok(Token::Prototype(entry)),
        None => Err(format!("unknown prototype `:{id}:`")),
    }
}

fn keyword(word: &str) -> Option<Keyword> {
    Some(match word {
        "making" => Keyword::Making,
        "in" => Keyword::In,
        "into" => Keyword::Into,
        "from" => Keyword::From,
        "loop" => Keyword::Loop,
        _ => return None,
    })
}

/// Whether `word` is reserved and therefore not a variable name (§4.4).
///
/// Unit keywords and control keywords never lex as identifiers, but the
/// numeric scale suffixes do, so a definition such as `M = 3` would otherwise
/// create a variable that can never be written in an expression.
/// A word that is never a variable name.
///
/// The scale suffixes (`k`, `M`, `G`, `T`) are not here: they only occur after
/// a number or before a unit, so `k` is an ordinary identifier (§4.4).
pub fn is_reserved_word(word: &str) -> bool {
    Unit::from_keyword(word).is_some() || keyword(word).is_some()
}

fn starts_number(rest: &str) -> bool {
    let mut chars = rest.chars();
    chars.next() == Some('.') && chars.next().is_some_and(|c| c.is_ascii_digit())
}

fn scan_number(line: &str, start: usize) -> Result<(f64, usize), ParseError> {
    let bytes = line.as_bytes();
    let mut index = start;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
    }
    let text = &line[start..index];
    if text == "." || text.ends_with('.') {
        return Err(ParseError::new(
            Span::new(start, index),
            "a number cannot end with `.`",
        ));
    }
    match text.parse::<f64>() {
        Ok(value) => Ok((value, index)),
        Err(_) => Err(ParseError::new(
            Span::new(start, index),
            format!("`{text}` is not a number"),
        )),
    }
}

fn scale_suffix(word: &str) -> Option<f64> {
    Some(match word {
        "k" => 1e3,
        "M" => 1e6,
        "G" => 1e9,
        "T" => 1e12,
        _ => return None,
    })
}

fn scan_identifier(line: &str, start: usize) -> usize {
    let mut index = start;
    for ch in line[start..].chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    index
}

/// Scans `:name:` starting at the colon, returning the id and the end offset.
fn scan_token_body(line: &str, start: usize) -> Result<(String, usize), ParseError> {
    let rest = &line[start + 1..];
    let Some(end) = rest.find(':') else {
        return Err(ParseError::new(
            Span::new(start, line.len()),
            "unterminated token",
        ));
    };
    if end == 0 {
        return Err(ParseError::new(Span::new(start, start + 2), "empty token"));
    }
    Ok((rest[..end].to_string(), start + 1 + end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::test_registry;

    fn tokens(line: &str) -> Vec<Token> {
        lex(line, &test_registry())
            .expect("lexes")
            .into_iter()
            .map(|(token, _)| token)
            .collect()
    }

    #[test]
    fn adjacent_unit_keywords_are_separate_tokens() {
        assert_eq!(
            tokens("3s"),
            vec![Token::Number(3.0), Token::Unit(Unit::Second(1.0))]
        );
        assert_eq!(
            tokens("3 kJ"),
            vec![Token::Number(3.0), Token::Unit(Unit::Joule(1e3))]
        );
    }

    #[test]
    fn scale_suffixes_fold_into_the_number() {
        assert_eq!(tokens("1k"), vec![Token::Number(1000.0)]);
        assert_eq!(tokens("1M"), vec![Token::Number(1e6)]);
    }

    #[test]
    fn a_word_may_not_follow_a_number_without_a_space() {
        assert!(lex("3x", &test_registry()).is_err());
        assert!(lex("3 x", &test_registry()).is_ok());
    }

    #[test]
    fn a_quality_before_a_prototype_is_one_token() {
        let registry = test_registry();
        let iron = registry.entry_id("iron-plate").unwrap();
        assert_eq!(
            lex(":rare: :iron-plate:", &registry)
                .unwrap()
                .into_iter()
                .map(|(token, _)| token)
                .collect::<Vec<_>>(),
            vec![Token::Qualified {
                id: iron,
                quality: Quality::Rare
            }]
        );
        assert_eq!(
            lex(":rare: 2", &registry)
                .unwrap()
                .into_iter()
                .map(|(token, _)| token)
                .collect::<Vec<_>>(),
            vec![Token::Quality(Quality::Rare), Token::Number(2.0)]
        );
    }

    #[test]
    fn comments_end_the_line() {
        assert_eq!(tokens("1 + 2 # three"), tokens("1 + 2"));
    }

    #[test]
    fn unknown_prototypes_are_errors() {
        let error = lex(":nope:", &test_registry()).unwrap_err();
        assert!(error.message.contains("unknown prototype"));
    }
}
