//! Parser for the facet language.
//!
//! This parser is intentionally separate from the legacy parser while the
//! evaluator is migrated. It produces a syntax tree only; token facets are
//! built by the evaluator against the registry.

use std::fmt;

use hactorio_data::{EntryId, Registry};

use crate::settings::TimeUnit;
use crate::syntax::{BinaryOp, Keyword, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    StackSize,
    StackedBelt,
    Time,
    Speed,
    Electric,
    ModuleSlots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Property {
    Rate,
    Capacity,
    StackSize,
    Power,
    Energy,
    Electric,
    Machines,
    Time,
    Crafts,
    Speed,
    Productivity,
    Prod,
    Pollution,
    Slots,
    Abs,
}

impl Property {
    /// The property a bare name selects, as in `value.rate`.
    pub fn from_name(name: &str) -> Option<Property> {
        Some(match name {
            "rate" => Property::Rate,
            "capacity" => Property::Capacity,
            "power" => Property::Power,
            "energy" => Property::Energy,
            "machines" => Property::Machines,
            "crafts" => Property::Crafts,
            "productivity" => Property::Productivity,
            "prod" => Property::Prod,
            "pollution" => Property::Pollution,
            "abs" => Property::Abs,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    Property(Property),
    Signal(Signal),
    Token(EntryId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Number(f64),
    Token(EntryId),
    Quantity {
        count: f64,
        token: EntryId,
    },
    Time {
        count: f64,
        unit: TimeUnit,
    },
    Joule(f64),
    Watt(f64),
    Craft(f64),
    Variable(String),
    Neg(Box<Expr>),
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Juxtapose {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    With {
        machine: Box<Expr>,
        config: Box<Expr>,
    },
    Making {
        machine: Box<Expr>,
        recipe: Box<Expr>,
    },
    In {
        quantity: Box<Expr>,
        machine: Box<Expr>,
    },
    StackSize(Box<Expr>),
    StackedBelt(Box<Expr>),
    Property {
        value: Box<Expr>,
        selector: Selector,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Blank,
    Comment,
    Expression(Expr),
    Definition {
        target: AssignmentTarget,
        op: DefinitionOp,
        value: Expr,
    },
}

/// What an assignment statement writes to.
#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentTarget {
    /// `x = ...`: rebind the variable.
    Variable(String),
    /// `p.power = ...`: mutate a facet of the process the variable holds.
    Property {
        variable: String,
        selector: Selector,
    },
}

/// How a definition combines the right-hand side with the name's current binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionOp {
    /// `=`: replace the binding.
    Assign,
    /// `+=`, `-=`, `*=`, `/=`.
    Compound(BinaryOp),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub span: Span,
    pub message: String,
}

impl ParseError {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        ParseError {
            span,
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    span: Span,
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    Number(f64),
    Prototype(EntryId),
    Signal(Signal),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Equals,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    LParen,
    RParen,
    Dot,
    In,
    With,
    Making,
    Time(TimeUnit),
    Joule(f64),
    Watt(f64),
    Craft,
}

impl TokenKind {
    fn describe(&self) -> String {
        match self {
            TokenKind::Number(_) => "a number".into(),
            TokenKind::Prototype(_) => "a prototype token".into(),
            TokenKind::Signal(_) => "a signal".into(),
            TokenKind::Ident(name) => format!("`{name}`"),
            TokenKind::Plus => "`+`".into(),
            TokenKind::Minus => "`-`".into(),
            TokenKind::Star => "`*`".into(),
            TokenKind::Slash => "`/`".into(),
            TokenKind::Caret => "`^`".into(),
            TokenKind::Equals => "`=`".into(),
            TokenKind::PlusEq => "`+=`".into(),
            TokenKind::MinusEq => "`-=`".into(),
            TokenKind::StarEq => "`*=`".into(),
            TokenKind::SlashEq => "`/=`".into(),
            TokenKind::LParen => "`(`".into(),
            TokenKind::RParen => "`)`".into(),
            TokenKind::Dot => "`.`".into(),
            TokenKind::In => "`in`".into(),
            TokenKind::With => "`with`".into(),
            TokenKind::Making => "`making`".into(),
            TokenKind::Time(unit) => format!("`{}`", unit.name()),
            TokenKind::Joule(_) => "`joule`".into(),
            TokenKind::Watt(_) => "`watt`".into(),
            TokenKind::Craft => "`craft`".into(),
        }
    }

    fn starts_primary(&self) -> bool {
        matches!(
            self,
            TokenKind::Number(_)
                | TokenKind::Prototype(_)
                | TokenKind::Signal(Signal::StackSize)
                | TokenKind::Signal(Signal::StackedBelt)
                | TokenKind::Ident(_)
                | TokenKind::LParen
                | TokenKind::Time(_)
                | TokenKind::Joule(_)
                | TokenKind::Watt(_)
                | TokenKind::Craft
        )
    }
}

pub fn parse_line(line: &str, registry: &Registry) -> Result<Statement, ParseError> {
    let tokens = lex(line, registry)?;
    if tokens.is_empty() {
        let kind = if line.trim_start().starts_with('#') {
            StatementKind::Comment
        } else {
            StatementKind::Blank
        };
        return Ok(Statement {
            kind,
            span: Span::new(0, line.len() as u32),
        });
    }

    let mut parser = Parser {
        tokens,
        pos: 0,
        registry,
    };
    parser.parse_statement()
}

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    registry: &'a Registry,
}

// Juxtaposition is tighter than `^` and `*`.
const BP_IN: u8 = 1;
const BP_ADD: u8 = 2;
const BP_MUL: u8 = 3;
const BP_NEG: u8 = 4;
const BP_POW: u8 = 5;
const BP_JUMP: u8 = 6;

impl Parser<'_> {
    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let first = self.tokens[0].span;
        // `parse_expr` stops at an assignment operator, so the left-hand side is
        // the assignment target when one follows.
        let lhs = self.parse_expr()?;
        if let Some(op) = self.peek_kind().and_then(definition_op) {
            let Some(target) = assignment_target(&lhs) else {
                return Err(ParseError::new(
                    lhs.span,
                    "assignment target must be a variable or a process net property",
                ));
            };
            self.validate_target(&target, lhs.span)?;
            self.pos += 1;
            let value = self.parse_expr()?;
            let span = Span::new(first.start, self.last_end());
            self.expect_end()?;
            return Ok(Statement {
                kind: StatementKind::Definition { target, op, value },
                span,
            });
        }

        let span = Span::new(first.start, self.last_end());
        self.expect_end()?;
        Ok(Statement {
            kind: StatementKind::Expression(lhs),
            span,
        })
    }

    fn validate_target(&self, target: &AssignmentTarget, span: Span) -> Result<(), ParseError> {
        let (variable, selector) = match target {
            AssignmentTarget::Variable(variable) => (variable, None),
            AssignmentTarget::Property { variable, selector } => (variable, Some(selector)),
        };
        if !is_valid_identifier(variable) {
            return Err(ParseError::new(
                span,
                format!("`{variable}` is not a valid identifier"),
            ));
        }
        if self.registry.entry(variable).is_some() {
            return Err(ParseError::new(
                span,
                format!("`{variable}` is a prototype id and cannot be a variable"),
            ));
        }
        if let Some(selector) = selector
            && !is_assignable_selector(selector)
        {
            return Err(ParseError::new(
                span,
                "only `.power`, `.:electric:`, `.crafts`, and item selectors are assignable",
            ));
        }
        Ok(())
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_bp(0, &[], true)
    }

    fn parse_bp(
        &mut self,
        min_bp: u8,
        stops: &[TokenKind],
        machine_ops: bool,
    ) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_machine_expr(machine_ops)?;

        loop {
            if self.at_stop(stops) {
                break;
            }
            let (infix, bp, binary_op) = match self.peek_kind() {
                Some(TokenKind::In) => (Infix::In, BP_IN, None),
                Some(TokenKind::Plus) => (Infix::Add, BP_ADD, Some(BinaryOp::Add)),
                Some(TokenKind::Minus) => (Infix::Add, BP_ADD, Some(BinaryOp::Sub)),
                Some(TokenKind::Star) => (Infix::Mul, BP_MUL, Some(BinaryOp::Mul)),
                Some(TokenKind::Slash) => (Infix::Mul, BP_MUL, Some(BinaryOp::Div)),
                Some(TokenKind::Caret) => (Infix::Pow, BP_POW, Some(BinaryOp::Pow)),
                Some(kind) if kind.starts_primary() => (Infix::Juxtapose, BP_JUMP, None),
                _ => break,
            };
            if bp < min_bp {
                break;
            }

            if infix == Infix::In {
                let span = self.peek_span().unwrap();
                self.pos += 1;
                let machine = self.parse_machine_expr(true)?;
                let start = lhs.span.start;
                lhs = Expr {
                    kind: ExprKind::In {
                        quantity: Box::new(lhs),
                        machine: Box::new(machine),
                    },
                    span: Span::new(start, self.last_end()),
                };
                if matches!(self.peek_kind(), Some(TokenKind::In)) {
                    return Err(ParseError::new(span, "`in` is not associative"));
                }
                continue;
            }

            if infix != Infix::Juxtapose {
                self.pos += 1;
            }
            let right_associative = infix == Infix::Pow;
            let rhs = self.parse_bp(
                if right_associative { bp } else { bp + 1 },
                stops,
                machine_ops,
            )?;
            let start = lhs.span.start;
            lhs = match binary_op {
                Some(op) => Expr {
                    kind: ExprKind::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    span: Span::new(start, self.last_end()),
                },
                None => Expr {
                    kind: ExprKind::Juxtapose {
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    span: Span::new(start, self.last_end()),
                },
            };
        }

        Ok(lhs)
    }

    fn parse_machine_expr(&mut self, machine_ops: bool) -> Result<Expr, ParseError> {
        let mut value = self.parse_primary(machine_ops)?;
        if !machine_ops {
            return Ok(value);
        }
        loop {
            match self.peek_kind() {
                Some(TokenKind::With) => {
                    self.pos += 1;
                    let config = self.parse_bp(
                        0,
                        &[
                            TokenKind::With,
                            TokenKind::Making,
                            TokenKind::In,
                            TokenKind::RParen,
                        ],
                        false,
                    )?;
                    let start = value.span.start;
                    value = Expr {
                        kind: ExprKind::With {
                            machine: Box::new(value),
                            config: Box::new(config),
                        },
                        span: Span::new(start, self.last_end()),
                    };
                }
                Some(TokenKind::Making) => {
                    self.pos += 1;
                    let recipe = self.parse_primary(true)?;
                    let start = value.span.start;
                    value = Expr {
                        kind: ExprKind::Making {
                            machine: Box::new(value),
                            recipe: Box::new(recipe),
                        },
                        span: Span::new(start, self.last_end()),
                    };
                    break;
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn parse_primary(&mut self, machine_ops: bool) -> Result<Expr, ParseError> {
        let token = self
            .advance()
            .ok_or_else(|| self.error_at_end("expected an expression"))?;
        let span = token.span;
        let mut value = match token.kind {
            TokenKind::Number(value) => self.after_number(value, span)?,
            TokenKind::Minus => {
                let inner = self.parse_bp(BP_NEG, &[], machine_ops)?;
                Expr {
                    kind: ExprKind::Neg(Box::new(inner)),
                    span: Span::new(span.start, self.last_end()),
                }
            }
            TokenKind::LParen => {
                let inner = self.parse_bp(0, &[], machine_ops)?;
                self.expect(TokenKind::RParen, "expected `)`")?;
                inner
            }
            TokenKind::Prototype(id) => Expr {
                kind: ExprKind::Token(id),
                span,
            },
            TokenKind::Signal(Signal::StackSize) => {
                if matches!(self.peek_kind(), Some(TokenKind::Minus)) {
                    return Err(ParseError::new(
                        span,
                        "`:stack-size:` cannot take a unary minus; use parentheses",
                    ));
                }
                let inner = self.parse_primary(false)?;
                Expr {
                    kind: ExprKind::StackSize(Box::new(inner)),
                    span: Span::new(span.start, self.last_end()),
                }
            }
            TokenKind::Signal(Signal::StackedBelt) => {
                if matches!(self.peek_kind(), Some(TokenKind::Minus)) {
                    return Err(ParseError::new(
                        span,
                        "`:stacked-belt:` cannot take a unary minus; use parentheses",
                    ));
                }
                let inner = self.parse_primary(false)?;
                Expr {
                    kind: ExprKind::StackedBelt(Box::new(inner)),
                    span: Span::new(span.start, self.last_end()),
                }
            }
            TokenKind::Signal(_) => {
                return Err(ParseError::new(span, "signals are only valid after `.`"));
            }
            TokenKind::Ident(name) if !is_valid_identifier(&name) => {
                return Err(ParseError::new(
                    span,
                    format!("`{name}` is not an identifier"),
                ));
            }
            TokenKind::Ident(name) => Expr {
                kind: ExprKind::Variable(name),
                span,
            },
            TokenKind::Time(unit) => Expr {
                kind: ExprKind::Time { count: 1.0, unit },
                span,
            },
            TokenKind::Joule(scale) => Expr {
                kind: ExprKind::Joule(scale),
                span,
            },
            TokenKind::Watt(scale) => Expr {
                kind: ExprKind::Watt(scale),
                span,
            },
            TokenKind::Craft => Expr {
                kind: ExprKind::Craft(1.0),
                span,
            },
            other => {
                return Err(ParseError::new(
                    span,
                    format!("expected an expression, found {}", other.describe()),
                ));
            }
        };

        while matches!(self.peek_kind(), Some(TokenKind::Dot)) {
            self.pos += 1;
            let selector = self.parse_selector()?;
            let start = value.span.start;
            value = Expr {
                kind: ExprKind::Property {
                    value: Box::new(value),
                    selector,
                },
                span: Span::new(start, self.last_end()),
            };
        }

        Ok(value)
    }

    fn after_number(&mut self, count: f64, span: Span) -> Result<Expr, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::Time(unit)) => {
                let unit = *unit;
                self.pos += 1;
                Ok(Expr {
                    kind: ExprKind::Time { count, unit },
                    span: Span::new(span.start, self.last_end()),
                })
            }
            Some(TokenKind::Joule(scale)) => {
                let scale = *scale;
                self.pos += 1;
                Ok(Expr {
                    kind: ExprKind::Joule(count * scale),
                    span: Span::new(span.start, self.last_end()),
                })
            }
            Some(TokenKind::Watt(scale)) => {
                let scale = *scale;
                self.pos += 1;
                Ok(Expr {
                    kind: ExprKind::Watt(count * scale),
                    span: Span::new(span.start, self.last_end()),
                })
            }
            Some(TokenKind::Craft) => {
                self.pos += 1;
                Ok(Expr {
                    kind: ExprKind::Craft(count),
                    span: Span::new(span.start, self.last_end()),
                })
            }
            Some(TokenKind::Prototype(id)) => {
                let id = *id;
                self.pos += 1;
                Ok(Expr {
                    kind: ExprKind::Quantity { count, token: id },
                    span: Span::new(span.start, self.last_end()),
                })
            }
            _ => Ok(Expr {
                kind: ExprKind::Number(count),
                span,
            }),
        }
    }

    fn parse_selector(&mut self) -> Result<Selector, ParseError> {
        let token = self
            .advance()
            .ok_or_else(|| self.error_at_end("expected a property after `.`"))?;
        match token.kind {
            TokenKind::Prototype(id) => Ok(Selector::Token(id)),
            TokenKind::Signal(signal) => Ok(Selector::Signal(signal)),
            TokenKind::Ident(name) => property(&name)
                .map(Selector::Property)
                .ok_or_else(|| ParseError::new(token.span, format!("unknown property `{name}`"))),
            other => Err(ParseError::new(
                token.span,
                format!("expected a property, found {}", other.describe()),
            )),
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<(), ParseError> {
        match self.peek_kind() {
            Some(found) if *found == kind => {
                self.pos += 1;
                Ok(())
            }
            _ => Err(self.error_at_end(message)),
        }
    }

    fn expect_end(&mut self) -> Result<(), ParseError> {
        if let Some(token) = self.peek() {
            return Err(ParseError::new(
                token.span,
                format!("unexpected token {}", token.kind.describe()),
            ));
        }
        Ok(())
    }

    fn at_stop(&self, stops: &[TokenKind]) -> bool {
        self.peek_kind().is_some_and(|kind| stops.contains(kind))
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|token| &token.kind)
    }

    fn peek_span(&self) -> Option<Span> {
        self.peek().map(|token| token.span)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?.clone();
        self.pos += 1;
        Some(token)
    }

    fn last_end(&self) -> u32 {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map(|token| token.span.end)
            .unwrap_or(0)
    }

    fn error_at_end(&self, message: &str) -> ParseError {
        let span = self
            .tokens
            .last()
            .map(|token| Span::new(token.span.end, token.span.end))
            .unwrap_or(Span::new(0, 0));
        ParseError::new(span, message)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Infix {
    Add,
    Mul,
    Pow,
    Juxtapose,
    In,
}

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// The assignment target an assignment's left-hand side denotes, if any.
fn assignment_target(expr: &Expr) -> Option<AssignmentTarget> {
    match &expr.kind {
        ExprKind::Variable(name) => Some(AssignmentTarget::Variable(name.clone())),
        ExprKind::Property { value, selector } => match &value.kind {
            ExprKind::Variable(name) => Some(AssignmentTarget::Property {
                variable: name.clone(),
                selector: selector.clone(),
            }),
            _ => None,
        },
        _ => None,
    }
}

fn definition_op(kind: &TokenKind) -> Option<DefinitionOp> {
    Some(match kind {
        TokenKind::Equals => DefinitionOp::Assign,
        TokenKind::PlusEq => DefinitionOp::Compound(BinaryOp::Add),
        TokenKind::MinusEq => DefinitionOp::Compound(BinaryOp::Sub),
        TokenKind::StarEq => DefinitionOp::Compound(BinaryOp::Mul),
        TokenKind::SlashEq => DefinitionOp::Compound(BinaryOp::Div),
        _ => return None,
    })
}

fn property(name: &str) -> Option<Property> {
    Property::from_name(name)
}

/// Whether a selector can be the target of an assignment.
fn is_assignable_selector(selector: &Selector) -> bool {
    match selector {
        Selector::Property(Property::Power | Property::Crafts) => true,
        Selector::Signal(Signal::Electric) => true,
        Selector::Token(_) => true,
        Selector::Property(_) | Selector::Signal(_) => false,
    }
}

fn keyword(word: &str) -> Option<TokenKind> {
    Some(match crate::syntax::keyword(word)? {
        Keyword::In => TokenKind::In,
        Keyword::With => TokenKind::With,
        Keyword::Making => TokenKind::Making,
        Keyword::Joule(multiplier) => TokenKind::Joule(multiplier),
        Keyword::Watt(multiplier) => TokenKind::Watt(multiplier),
        Keyword::Craft => TokenKind::Craft,
        Keyword::Time(unit) => TokenKind::Time(unit),
    })
}

fn lex(line: &str, registry: &Registry) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < line.len() {
        let ch = next_char(line, index);
        match ch {
            ' ' | '\t' | '\r' => index += 1,
            '#' => break,
            '+' => push_operator(
                &mut tokens,
                line,
                &mut index,
                TokenKind::Plus,
                TokenKind::PlusEq,
            ),
            '-' => push_operator(
                &mut tokens,
                line,
                &mut index,
                TokenKind::Minus,
                TokenKind::MinusEq,
            ),
            '*' => push_operator(
                &mut tokens,
                line,
                &mut index,
                TokenKind::Star,
                TokenKind::StarEq,
            ),
            '/' => push_operator(
                &mut tokens,
                line,
                &mut index,
                TokenKind::Slash,
                TokenKind::SlashEq,
            ),
            '^' => push(&mut tokens, TokenKind::Caret, &mut index),
            '=' => push(&mut tokens, TokenKind::Equals, &mut index),
            '(' => push(&mut tokens, TokenKind::LParen, &mut index),
            ')' => push(&mut tokens, TokenKind::RParen, &mut index),
            '.' if is_digit(next_char_at(line, index + 1)) => {
                let start = index;
                let value = lex_number(line, &mut index)?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    span: Span::new(start as u32, index as u32),
                });
            }
            '.' => push(&mut tokens, TokenKind::Dot, &mut index),
            '0'..='9' => {
                let start = index;
                let value = lex_number(line, &mut index)?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    span: Span::new(start as u32, index as u32),
                });
            }
            ':' => {
                let start = index;
                tokens.push(lex_prototype_or_signal(line, &mut index, registry)?);
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
                // `stack-size` is the one hyphenated property name.
                if &line[start..index] == "stack" && line[index..].starts_with("-size") {
                    index += 5;
                }
                let word = &line[start..index];
                let span = Span::new(start as u32, index as u32);
                let kind = keyword(word).unwrap_or_else(|| TokenKind::Ident(word.to_string()));
                tokens.push(Token { kind, span });
            }
            _ if is_pua(ch) => {
                let start = index;
                index += ch.len_utf8();
                let span = Span::new(start as u32, index as u32);
                if let Some(signal) = hactorio_data::signal_by_glyph(ch) {
                    let kind = match signal.id {
                        "stack-size" => Signal::StackSize,
                        "stacked-belt" => Signal::StackedBelt,
                        "time" => Signal::Time,
                        "speed" => Signal::Speed,
                        "electric" => Signal::Electric,
                        "module-slots" => Signal::ModuleSlots,
                        _ => return Err(ParseError::new(span, "unknown signal glyph")),
                    };
                    tokens.push(Token {
                        kind: TokenKind::Signal(kind),
                        span,
                    });
                    continue;
                }
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

fn lex_prototype_or_signal(
    line: &str,
    index: &mut usize,
    registry: &Registry,
) -> Result<Token, ParseError> {
    let start = *index;
    *index += 1;
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
    *index += 1;
    let span = Span::new(start as u32, *index as u32);
    if id.is_empty() {
        return Err(ParseError::new(span, "empty prototype token"));
    }
    if let Some(signal) = signal(id) {
        return Ok(Token {
            kind: TokenKind::Signal(signal),
            span,
        });
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

fn signal(id: &str) -> Option<Signal> {
    Some(match id {
        "stack-size" => Signal::StackSize,
        "stacked-belt" => Signal::StackedBelt,
        "time" => Signal::Time,
        "speed" => Signal::Speed,
        "electric" => Signal::Electric,
        "module-slots" => Signal::ModuleSlots,
        _ => return None,
    })
}

fn push(tokens: &mut Vec<Token>, kind: TokenKind, index: &mut usize) {
    let start = *index;
    *index += 1;
    tokens.push(Token {
        kind,
        span: Span::new(start as u32, *index as u32),
    });
}

/// Pushes `plain`, or `compound` when the operator is immediately followed by `=`.
fn push_operator(
    tokens: &mut Vec<Token>,
    line: &str,
    index: &mut usize,
    plain: TokenKind,
    compound: TokenKind,
) {
    let start = *index;
    *index += 1;
    let kind = if line[*index..].starts_with('=') {
        *index += 1;
        compound
    } else {
        plain
    };
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
    let text = &line[start..*index];
    if text.ends_with('.') {
        return Err(ParseError::new(
            Span::new(start as u32, *index as u32),
            "a number cannot end with `.`",
        ));
    }
    let mut value: f64 = text
        .parse()
        .map_err(|_| ParseError::new(Span::new(start as u32, *index as u32), "invalid number"))?;
    if *index < line.len() {
        let multiplier = match next_char(line, *index) {
            'k' => Some(1e3),
            'M' => Some(1e6),
            'G' => Some(1e9),
            'T' => Some(1e12),
            _ => None,
        };
        if let Some(multiplier) = multiplier {
            value *= multiplier;
            *index += 1;
        }
    }
    if *index < line.len() {
        let ch = next_char(line, *index);
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = *index;
            let mut end = *index;
            while end < line.len() {
                let ch = next_char(line, end);
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    end += ch.len_utf8();
                } else {
                    break;
                }
            }
            let word = &line[start..end];
            let is_unit = matches!(
                keyword(word),
                Some(
                    TokenKind::Time(_)
                        | TokenKind::Joule(_)
                        | TokenKind::Watt(_)
                        | TokenKind::Craft
                )
            );
            if !is_unit {
                return Err(ParseError::new(
                    Span::new(start as u32, end as u32),
                    "a variable cannot follow a number without whitespace",
                ));
            }
        }
    }
    Ok(value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use hactorio_data::{Locale, postprocess};

    const DUMP: &str = r#"{
      "item": {
        "iron-plate": {"type": "item", "name": "iron-plate", "stack_size": 100},
        "iron-ore": {"type": "item", "name": "iron-ore", "stack_size": 50},
        "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}},
        "foundry": {"type": "item", "name": "foundry", "stack_size": 20}
      },
      "fluid": {},
      "module": {
        "speed-module-3": {"type": "module", "name": "speed-module-3", "category": "speed", "tier": 3, "stack_size": 50, "effect": {"speed": 0.5}}
      },
      "recipe": {
        "iron-plate": {"type": "recipe", "name": "iron-plate", "categories": ["smelting"], "energy_required": 3.2, "ingredients": [{"type": "item", "name": "iron-ore", "amount": 1}], "results": [{"type": "item", "name": "iron-plate", "amount": 1}]},
        "casting-iron": {"type": "recipe", "name": "casting-iron", "categories": ["metallurgy"], "energy_required": 3.2, "results": [{"type": "item", "name": "iron-plate", "amount": 1}]}
      }
    }"#;

    fn registry() -> Registry {
        postprocess(DUMP, &Locale::empty()).unwrap()
    }

    fn parse(line: &str) -> Statement {
        parse_line(line, &registry()).unwrap()
    }

    fn expr(line: &str) -> Expr {
        match parse(line).kind {
            StatementKind::Expression(expr) => expr,
            other => panic!("expected expression, got {other:?}"),
        }
    }

    #[test]
    fn parses_quantity_and_time_literals() {
        let ExprKind::Quantity { count, .. } = expr("3 :iron-plate:").kind else {
            panic!("expected quantity");
        };
        assert_eq!(count, 3.0);
        assert!(matches!(
            expr("3 s").kind,
            ExprKind::Time { count: 3.0, .. }
        ));
        assert!(matches!(expr("3 J").kind, ExprKind::Joule(3.0)));
        assert!(matches!(expr("3 W").kind, ExprKind::Watt(3.0)));
        assert!(matches!(expr("3J").kind, ExprKind::Joule(3.0)));
        assert!(matches!(expr("3W").kind, ExprKind::Watt(3.0)));
        assert!(matches!(expr("3kJ").kind, ExprKind::Joule(3_000.0)));
        assert!(matches!(expr("3kW").kind, ExprKind::Watt(3_000.0)));
        assert!(matches!(expr("3TW").kind, ExprKind::Watt(3e12)));
        assert!(matches!(expr("3 kJ").kind, ExprKind::Joule(3_000.0)));
        assert!(matches!(expr("3 MJ").kind, ExprKind::Joule(3e6)));
        assert!(matches!(expr("3 GJ").kind, ExprKind::Joule(3e9)));
        assert!(matches!(expr("3 TJ").kind, ExprKind::Joule(3e12)));
        assert!(matches!(expr("3 kW").kind, ExprKind::Watt(3_000.0)));
        assert!(matches!(expr("3 MW").kind, ExprKind::Watt(3e6)));
        assert!(matches!(expr("3 GW").kind, ExprKind::Watt(3e9)));
        assert!(matches!(expr("kJ").kind, ExprKind::Joule(1_000.0)));
        assert!(matches!(expr("TW").kind, ExprKind::Watt(1e12)));
        assert!(matches!(
            expr("3T").kind,
            ExprKind::Number(value) if value == 3e12
        ));
    }

    #[test]
    fn rejects_variables_immediately_after_numbers() {
        assert!(parse_line("3x", &registry()).is_err());
        assert!(parse_line("3kx", &registry()).is_err());
        assert!(parse_line("3.5x", &registry()).is_err());
        assert!(parse_line("3 x", &registry()).is_ok());
        assert!(parse_line("3k x", &registry()).is_ok());
    }

    #[test]
    fn parses_juxtaposed_prefix_operators() {
        assert!(matches!(
            expr("2 :stacked-belt: :iron-plate:").kind,
            ExprKind::Juxtapose { .. }
        ));
        assert!(matches!(
            expr("2 :stack-size: :iron-plate:").kind,
            ExprKind::Juxtapose { .. }
        ));
    }

    #[test]
    fn rejects_trailing_dot_numbers() {
        assert!(parse_line("3.", &registry()).is_err());
        assert!(parse_line("3. + 1", &registry()).is_err());
        assert!(parse_line(".5", &registry()).is_ok());
        assert!(parse_line("3.5", &registry()).is_ok());
    }

    #[test]
    fn rejects_stack_size_as_an_identifier() {
        assert!(parse_line("stack-size = 7", &registry()).is_err());
        assert!(parse_line("stack-size", &registry()).is_err());
        assert!(parse_line("x.stack-size", &registry()).is_err());
        assert!(parse_line("x.slots", &registry()).is_err());
        assert!(parse_line("x.:stack-size:", &registry()).is_ok());
        assert!(parse_line(".:stack-size:", &registry()).is_err());
    }

    #[test]
    fn lexes_signal_glyphs() {
        let time = hactorio_data::signal_by_id("time").unwrap().glyph;
        let time_expr = expr(&format!("x.{time}"));
        assert!(matches!(
            time_expr.kind,
            ExprKind::Property {
                selector: Selector::Signal(Signal::Time),
                ..
            }
        ));
        let stack = hactorio_data::signal_by_id("stack-size").unwrap().glyph;
        let stack_expr = expr(&format!("x.{stack}"));
        assert!(matches!(
            stack_expr.kind,
            ExprKind::Property {
                selector: Selector::Signal(Signal::StackSize),
                ..
            }
        ));
        let stacked = hactorio_data::signal_by_id("stacked-belt").unwrap().glyph;
        assert!(matches!(
            expr(&format!("{stacked} :iron-plate:")).kind,
            ExprKind::StackedBelt(_)
        ));
        assert!(matches!(
            expr(":stacked-belt: :iron-plate:").kind,
            ExprKind::StackedBelt(_)
        ));
    }

    #[test]
    fn parses_explicit_and_implicit_multiplication() {
        assert!(matches!(
            expr("3 :iron-plate:/s * (5 + 3) s").kind,
            ExprKind::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
        assert!(matches!(
            expr("2 :iron-plate: 3 s").kind,
            ExprKind::Juxtapose { .. }
        ));
    }

    #[test]
    fn parses_with_making_and_in_grouping() {
        assert!(matches!(
            expr("ma with sm making r").kind,
            ExprKind::Making { .. }
        ));
        assert!(matches!(
            expr("2 * ma making r").kind,
            ExprKind::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
        assert!(matches!(
            expr("ma making r * 2").kind,
            ExprKind::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
        assert!(matches!(
            expr("q in ma * 2").kind,
            ExprKind::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    }

    #[test]
    fn parses_stack_size_prefix_and_properties() {
        assert!(matches!(
            expr(":stack-size: b1 :iron-plate:").kind,
            ExprKind::Juxtapose { lhs, .. } if matches!(lhs.kind, ExprKind::StackSize(_))
        ));
        assert!(matches!(
            expr("b1.:stack-size:").kind,
            ExprKind::Property {
                selector: Selector::Signal(Signal::StackSize),
                ..
            }
        ));
        assert!(matches!(
            expr("p.:iron-plate:").kind,
            ExprKind::Property {
                selector: Selector::Token(_),
                ..
            }
        ));
    }

    #[test]
    fn stack_size_rejects_unparenthesized_unary_minus() {
        assert!(parse_line(":stack-size: -b1", &registry()).is_err());
        assert!(parse_line(":stack-size: (-b1)", &registry()).is_ok());
    }

    #[test]
    fn parses_property_assignment_targets() {
        let StatementKind::Definition { target, op, .. } = parse("p.power = 1 W").kind else {
            panic!("expected definition");
        };
        assert_eq!(op, DefinitionOp::Assign);
        assert_eq!(
            target,
            AssignmentTarget::Property {
                variable: "p".to_string(),
                selector: Selector::Property(Property::Power),
            }
        );

        let StatementKind::Definition { target, op, .. } =
            parse("p.:iron-plate: += 1 :iron-plate:/s").kind
        else {
            panic!("expected definition");
        };
        assert_eq!(op, DefinitionOp::Compound(BinaryOp::Add));
        let AssignmentTarget::Property {
            variable,
            selector: Selector::Token(_),
        } = target
        else {
            panic!("expected item selector, got {target:?}");
        };
        assert_eq!(variable, "p");

        assert!(matches!(
            parse("p.crafts = 1/s").kind,
            StatementKind::Definition {
                target: AssignmentTarget::Property {
                    selector: Selector::Property(Property::Crafts),
                    ..
                },
                op: DefinitionOp::Assign,
                ..
            }
        ));
        assert!(matches!(
            parse("p.:electric: = 100 kJ").kind,
            StatementKind::Definition {
                target: AssignmentTarget::Property {
                    selector: Selector::Signal(Signal::Electric),
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn rejects_non_assignable_property_targets() {
        for line in [
            "p.rate = 1",
            "(p + q).power = 1",
            "1 = 2",
            "p.power.power = 1",
        ] {
            assert!(parse_line(line, &registry()).is_err(), "{line}");
        }
    }

    #[test]
    fn parses_definitions() {
        assert!(matches!(
            parse("x = 3 :iron-plate:").kind,
            StatementKind::Definition { .. }
        ));
    }

    #[test]
    fn parses_compound_definitions() {
        assert!(matches!(
            parse("x = 3").kind,
            StatementKind::Definition {
                op: DefinitionOp::Assign,
                ..
            }
        ));
        for (line, op) in [
            ("x += 3", BinaryOp::Add),
            ("x -= 3", BinaryOp::Sub),
            ("x *= 3", BinaryOp::Mul),
            ("x /= 3", BinaryOp::Div),
        ] {
            assert!(
                matches!(
                    parse(line).kind,
                    StatementKind::Definition {
                        op: DefinitionOp::Compound(found),
                        ..
                    } if found == op
                ),
                "{line}"
            );
        }
        assert!(matches!(
            parse("x+=3").kind,
            StatementKind::Definition {
                op: DefinitionOp::Compound(BinaryOp::Add),
                ..
            }
        ));
        // The operator characters must be adjacent.
        assert!(parse_line("x + = 3", &registry()).is_err());
        // Compound assignment is statements-only.
        assert!(parse_line("y = x += 1", &registry()).is_err());
    }
}
