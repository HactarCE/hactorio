//! Tokenizer for one line.

use hactorio_data::{EntryId, Registry};

use crate::ast::Span;
use crate::belt::BeltTier;
use crate::parser::ParseError;
use crate::settings::TimeUnit;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    Number(f64),
    Prototype(EntryId),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Equals,
    LParen,
    RParen,
    At,
    In,
    Making,
    Time(TimeUnit),
    Joule,
    Watt,
    Craft,
    Belt { tier: BeltTier, stacked: bool },
}

impl TokenKind {
    pub(crate) fn describe(&self) -> String {
        match self {
            TokenKind::Number(_) => "a number".into(),
            TokenKind::Prototype(_) => "a prototype token".into(),
            TokenKind::Ident(name) => format!("`{name}`"),
            TokenKind::Plus => "`+`".into(),
            TokenKind::Minus => "`-`".into(),
            TokenKind::Star => "`*`".into(),
            TokenKind::Slash => "`/`".into(),
            TokenKind::Caret => "`^`".into(),
            TokenKind::Equals => "`=`".into(),
            TokenKind::LParen => "`(`".into(),
            TokenKind::RParen => "`)`".into(),
            TokenKind::At => "`@`".into(),
            TokenKind::In => "`in`".into(),
            TokenKind::Making => "`making`".into(),
            TokenKind::Time(unit) => format!("`{}`", unit.name()),
            TokenKind::Joule => "`joule`".into(),
            TokenKind::Watt => "`watt`".into(),
            TokenKind::Craft => "`craft`".into(),
            TokenKind::Belt { .. } => "a belt token".into(),
        }
    }
}

/// Splits a line into tokens. A `#` ends the line, so comments produce no tokens.
pub(crate) fn lex(line: &str, registry: &Registry) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < line.len() {
        let ch = next_char(line, index);
        match ch {
            ' ' | '\t' | '\r' => index += 1,
            '#' => break,
            '+' => push(&mut tokens, TokenKind::Plus, &mut index),
            '-' => push(&mut tokens, TokenKind::Minus, &mut index),
            '*' => push(&mut tokens, TokenKind::Star, &mut index),
            '/' => push(&mut tokens, TokenKind::Slash, &mut index),
            '^' => push(&mut tokens, TokenKind::Caret, &mut index),
            '=' => push(&mut tokens, TokenKind::Equals, &mut index),
            '(' => push(&mut tokens, TokenKind::LParen, &mut index),
            ')' => push(&mut tokens, TokenKind::RParen, &mut index),
            '@' => push(&mut tokens, TokenKind::At, &mut index),
            '0'..='9' => {
                let start = index;
                let value = lex_number(line, &mut index)?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    span: Span::new(start as u32, index as u32),
                });
            }
            '.' if is_digit(next_char_at(line, index + 1)) => {
                let start = index;
                let value = lex_number(line, &mut index)?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    span: Span::new(start as u32, index as u32),
                });
            }
            ':' => {
                let start = index;
                tokens.push(lex_prototype(line, &mut index, registry)?);
                debug_assert!(index > start);
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = index;
                while index < line.len() {
                    let ch = next_char(line, index);
                    if ch.is_ascii_alphanumeric() || ch == '_' {
                        index += 1;
                    } else {
                        break;
                    }
                }
                let word = &line[start..index];
                let span = Span::new(start as u32, index as u32);
                let kind = match keyword(word) {
                    Some(kind) => kind,
                    None if matches!(word, "k" | "M" | "G") => {
                        return Err(ParseError::new(
                            span,
                            "`k`, `M`, and `G` are numeric suffixes, not tokens",
                        ));
                    }
                    None => TokenKind::Ident(word.to_string()),
                };
                tokens.push(Token { kind, span });
            }
            _ if is_pua(ch) => {
                let start = index;
                index += ch.len_utf8();
                let span = Span::new(start as u32, index as u32);
                let entry = registry
                    .entry_by_glyph(ch)
                    .ok_or_else(|| ParseError::new(span, "unknown prototype glyph"))?;
                let id = registry
                    .entry_id(&entry.id)
                    .ok_or_else(|| ParseError::new(span, "unknown prototype glyph"))?;
                tokens.push(Token {
                    kind: TokenKind::Prototype(id),
                    span,
                });
            }
            _ => {
                let start = index;
                index += ch.len_utf8();
                return Err(ParseError::new(
                    Span::new(start as u32, index as u32),
                    format!("unexpected character `{ch}`"),
                ));
            }
        }
    }

    Ok(tokens)
}

fn push(tokens: &mut Vec<Token>, kind: TokenKind, index: &mut usize) {
    let start = *index;
    *index += 1;
    tokens.push(Token {
        kind,
        span: Span::new(start as u32, *index as u32),
    });
}

fn lex_number(line: &str, index: &mut usize) -> Result<f64, ParseError> {
    let start = *index;
    let mut seen_dot = false;
    while *index < line.len() {
        let ch = next_char(line, *index);
        if ch.is_ascii_digit() {
            *index += 1;
        } else if ch == '.' && !seen_dot {
            seen_dot = true;
            *index += 1;
        } else {
            break;
        }
    }
    let mut text = line[start..*index].to_string();
    if text.ends_with('.') {
        text.push('0');
    }
    let mut value: f64 = text
        .parse()
        .map_err(|_| ParseError::new(Span::new(start as u32, *index as u32), "invalid number"))?;

    if *index < line.len() {
        let ch = next_char(line, *index);
        let multiplier = match ch {
            'k' => Some(1e3),
            'M' => Some(1e6),
            'G' => Some(1e9),
            _ => None,
        };
        if let Some(multiplier) = multiplier {
            value *= multiplier;
            *index += 1;
        }
    }
    Ok(value)
}

fn lex_prototype(
    line: &str,
    index: &mut usize,
    registry: &Registry,
) -> Result<Token, ParseError> {
    let start = *index;
    *index += 1; // ':'
    let id_start = *index;
    while *index < line.len() && next_char(line, *index) != ':' {
        *index += next_char(line, *index).len_utf8();
    }
    if *index >= line.len() {
        return Err(ParseError::new(
            Span::new(start as u32, *index as u32),
            "unterminated prototype token",
        ));
    }
    let id = &line[id_start..*index];
    *index += 1; // closing ':'
    let span = Span::new(start as u32, *index as u32);
    if id.is_empty() {
        return Err(ParseError::new(span, "empty prototype token"));
    }
    let entry = registry
        .entry(id)
        .ok_or_else(|| ParseError::new(span, format!("unknown prototype `{id}`")))?;
    let id = registry
        .entry_id(&entry.id)
        .ok_or_else(|| ParseError::new(span, format!("unknown prototype `{id}`")))?;
    Ok(Token {
        kind: TokenKind::Prototype(id),
        span,
    })
}

fn keyword(word: &str) -> Option<TokenKind> {
    let belt = |tier, stacked| Some(TokenKind::Belt { tier, stacked });
    match word {
        "in" => Some(TokenKind::In),
        "making" => Some(TokenKind::Making),
        "joule" => Some(TokenKind::Joule),
        "watt" => Some(TokenKind::Watt),
        "craft" => Some(TokenKind::Craft),
        "yb" => belt(BeltTier::Yellow, false),
        "rb" => belt(BeltTier::Red, false),
        "bb" => belt(BeltTier::Blue, false),
        "gb" => belt(BeltTier::Green, false),
        "syb" => belt(BeltTier::Yellow, true),
        "srb" => belt(BeltTier::Red, true),
        "sbb" => belt(BeltTier::Blue, true),
        "sgb" => belt(BeltTier::Green, true),
        _ => TimeUnit::from_word(word).map(TokenKind::Time),
    }
}

fn next_char(line: &str, index: usize) -> char {
    line[index..].chars().next().unwrap_or('\0')
}

fn next_char_at(line: &str, index: usize) -> char {
    line.get(index..)
        .and_then(|rest| rest.chars().next())
        .unwrap_or('\0')
}

fn is_digit(ch: char) -> bool {
    ch.is_ascii_digit()
}

fn is_pua(ch: char) -> bool {
    (0xE000..=0xF8FF).contains(&(ch as u32))
}
