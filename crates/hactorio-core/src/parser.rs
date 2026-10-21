//! Hand-written Pratt parser for one line.

use std::fmt;

use hactorio_data::{EntryId, Registry};

use crate::ast::{BinaryOp, Expr, MachineSpec, ModuleUse, Span, Statement, StatementKind};
use crate::belt::BeltTier;
use crate::lexer::{Token, TokenKind, lex};
use crate::settings::TimeUnit;
use crate::value::{BaseUnit, Value};

// Binding powers, tightest last. Unary minus sits below `^` so `-2^2` is `-4`.
const BP_IN: u8 = 1;
const BP_ADD: u8 = 2;
const BP_MUL: u8 = 3;
const BP_NEG: u8 = 4;
const BP_POW: u8 = 5;

/// A parse failure, with the span of the offending token.
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

/// Parses one line against the registry.
///
/// The whole line must be consumed; trailing tokens are an error.
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

impl Parser<'_> {
    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let first = self.tokens[0].span;
        let is_definition = self
            .tokens
            .get(1)
            .is_some_and(|token| token.kind == TokenKind::Equals);

        if is_definition {
            let name = match &self.tokens[0].kind {
                TokenKind::Ident(name) => name.clone(),
                other => {
                    return Err(ParseError::new(
                        first,
                        format!(
                            "expected a variable name before `=`, found {}",
                            other.describe()
                        ),
                    ));
                }
            };
            if self.registry.entry(&name).is_some() {
                return Err(ParseError::new(
                    first,
                    format!("`{name}` is a prototype id and cannot be a variable"),
                ));
            }
            self.pos = 2;
            let value = self.parse_expr()?;
            let span = Span::new(first.start, self.last_end());
            self.expect_end()?;
            return Ok(Statement {
                kind: StatementKind::Definition { name, value },
                span,
            });
        }

        let value = self.parse_expr()?;
        let span = Span::new(first.start, self.last_end());
        self.expect_end()?;
        Ok(Statement {
            kind: StatementKind::Expression(value),
            span,
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_bp(0)
    }

    fn parse_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            let (infix, _) = self.infix();
            let bp = match infix {
                Infix::In => BP_IN,
                Infix::Add | Infix::Sub => BP_ADD,
                Infix::Mul | Infix::Div => BP_MUL,
                Infix::Pow => BP_POW,
                Infix::None => break,
            };
            if bp < min_bp {
                break;
            }

            if let Infix::In = infix {
                self.pos += 1;
                let machine = self.parse_bp(BP_NEG)?;
                lhs = Expr::In {
                    quantity: Box::new(lhs),
                    machine: Box::new(machine),
                };
                if let (Infix::In, span) = self.infix() {
                    return Err(ParseError::new(span, "`in` is not associative"));
                }
                continue;
            }

            self.pos += 1;
            let right_associative = matches!(infix, Infix::Pow);
            let rhs = self.parse_bp(if right_associative { bp } else { bp + 1 })?;
            let op = match infix {
                Infix::Add => BinaryOp::Add,
                Infix::Sub => BinaryOp::Sub,
                Infix::Mul => BinaryOp::Mul,
                Infix::Div => BinaryOp::Div,
                Infix::Pow => BinaryOp::Pow,
                Infix::In | Infix::None => unreachable!(),
            };
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ParseError> {
        let token = match self.advance() {
            Some(token) => token,
            None => return Err(self.error_at_end("expected an expression")),
        };
        let span = token.span;

        match token.kind {
            TokenKind::Number(value) => self.after_number(value, span),
            TokenKind::Minus => Ok(Expr::Neg(Box::new(self.parse_bp(BP_NEG)?))),
            TokenKind::LParen => {
                let inner = self.parse_expr()?;
                self.expect(TokenKind::RParen, "expected `)`")?;
                Ok(inner)
            }
            TokenKind::At => Ok(Expr::Machine(self.parse_machine_body(1.0)?)),
            TokenKind::Prototype(id) => self.prototype_operand(id, span),
            TokenKind::Ident(name) => Ok(Expr::Variable(name)),
            TokenKind::Time(unit) => Ok(Expr::Literal(unit.value(1.0))),
            TokenKind::Joule => Ok(Expr::Literal(Value::joules(1.0))),
            TokenKind::Watt => Ok(Expr::Literal(Value::watts(1.0))),
            TokenKind::Craft => Ok(Expr::Literal(Value::crafts(1.0))),
            TokenKind::Belt { tier, stacked } => self.belt_literal(1.0, tier, stacked, span),
            other => Err(ParseError::new(
                span,
                format!("expected an expression, found {}", other.describe()),
            )),
        }
    }

    /// A number is a plain scalar unless a juxtaposed literal follows it.
    fn after_number(&mut self, value: f64, span: Span) -> Result<Expr, ParseError> {
        match self.follow() {
            Follow::At => {
                positive_integer(value, span, "machine counts must be positive integers")?;
                self.pos += 1;
                Ok(Expr::Machine(self.parse_machine_body(value)?))
            }
            Follow::Belt { tier, stacked } => {
                self.pos += 1;
                self.belt_literal(value, tier, stacked, span)
            }
            Follow::Time(unit) => {
                self.pos += 1;
                Ok(Expr::Literal(unit.value(value)))
            }
            Follow::Joule => {
                self.pos += 1;
                Ok(Expr::Literal(Value::joules(value)))
            }
            Follow::Watt => {
                self.pos += 1;
                Ok(Expr::Literal(Value::watts(value)))
            }
            Follow::Craft => {
                self.pos += 1;
                Ok(Expr::Literal(Value::crafts(value)))
            }
            Follow::Prototype(id, span) => {
                self.pos += 1;
                self.prototype_quantity(id, span, value)
            }
            Follow::None => Ok(Expr::Literal(Value::scalar(value))),
        }
    }

    fn belt_literal(
        &mut self,
        count: f64,
        tier: BeltTier,
        stacked: bool,
        count_span: Span,
    ) -> Result<Expr, ParseError> {
        if !(count.is_finite() && count > 0.0) {
            return Err(ParseError::new(
                count_span,
                "belt counts must be positive numbers",
            ));
        }
        let token = self
            .advance()
            .ok_or_else(|| self.error_at_end("expected an item after the belt token"))?;
        let id = match token.kind {
            TokenKind::Prototype(id) => id,
            _ => {
                return Err(ParseError::new(
                    token.span,
                    "expected an item after the belt token",
                ));
            }
        };
        let entry = self.registry.entry_by_index(id);
        if entry.item.is_none() {
            return Err(ParseError::new(
                token.span,
                format!("belt literals need an item, not `{}`", entry.id),
            ));
        }
        Ok(Expr::Belt {
            count,
            tier,
            stacked,
            item: id,
        })
    }

    fn prototype_operand(&self, id: EntryId, span: Span) -> Result<Expr, ParseError> {
        self.prototype_quantity(id, span, 1.0)
    }

    /// A prototype token as a value: `:iron-plate:` or the `3 :iron-plate:`
    /// juxtaposition literal.
    fn prototype_quantity(
        &self,
        id: EntryId,
        span: Span,
        coefficient: f64,
    ) -> Result<Expr, ParseError> {
        let entry = self.registry.entry_by_index(id);
        if entry.item.is_some() {
            Ok(Expr::Literal(Value::new(
                coefficient,
                [(BaseUnit::Item(id), 1)],
            )))
        } else if entry.fluid {
            Ok(Expr::Literal(Value::new(
                coefficient,
                [(BaseUnit::Fluid(id), 1)],
            )))
        } else if entry.module.is_some() {
            Err(ParseError::new(
                span,
                format!(
                    "`{}` is a module; modules are only valid in a machine spec",
                    entry.id
                ),
            ))
        } else {
            Err(ParseError::new(
                span,
                format!(
                    "`{}` is a recipe; recipes are only valid after `making`",
                    entry.id
                ),
            ))
        }
    }

    /// Everything after `@`: the machine, its modules, and an optional recipe.
    fn parse_machine_body(&mut self, count: f64) -> Result<MachineSpec, ParseError> {
        let token = self
            .advance()
            .ok_or_else(|| self.error_at_end("expected a machine item after `@`"))?;
        let id = match token.kind {
            TokenKind::Prototype(id) => id,
            _ => {
                return Err(ParseError::new(
                    token.span,
                    "expected a machine item after `@`",
                ));
            }
        };
        let entry = self.registry.entry_by_index(id);
        if entry.machine().is_none() {
            return Err(ParseError::new(
                token.span,
                format!("`{}` is not a machine", entry.id),
            ));
        }

        let mut modules = Vec::new();
        let mut recipe = None;

        loop {
            let step = self.spec_step();
            match step {
                SpecStep::Done => break,
                SpecStep::Making => {
                    self.pos += 1;
                    let token = self
                        .advance()
                        .ok_or_else(|| self.error_at_end("expected a recipe after `making`"))?;
                    let id = match token.kind {
                        TokenKind::Prototype(id) => id,
                        _ => {
                            return Err(ParseError::new(
                                token.span,
                                "expected a recipe after `making`",
                            ));
                        }
                    };
                    let entry = self.registry.entry_by_index(id);
                    let Some(recipe_id) = entry.recipe else {
                        return Err(ParseError::new(
                            token.span,
                            format!("`{}` is not a recipe", entry.id),
                        ));
                    };
                    recipe = Some(recipe_id);
                    break;
                }
                SpecStep::Count(count, span) => {
                    positive_integer(count, span, "module counts must be positive integers")?;
                    self.pos += 1;
                    let token = self
                        .advance()
                        .ok_or_else(|| self.error_at_end("expected a module after its count"))?;
                    match token.kind {
                        TokenKind::Prototype(id) => {
                            modules.push(self.module_use(id, token.span, count)?);
                        }
                        _ => {
                            return Err(ParseError::new(
                                token.span,
                                "expected a module after its count",
                            ));
                        }
                    }
                }
                SpecStep::Prototype(id, span) => {
                    self.pos += 1;
                    modules.push(self.module_use(id, span, 1.0)?);
                }
            }
        }

        Ok(MachineSpec {
            machine: id,
            count,
            modules,
            recipe,
        })
    }

    fn module_use(&self, id: EntryId, span: Span, count: f64) -> Result<ModuleUse, ParseError> {
        let entry = self.registry.entry_by_index(id);
        if entry.module.is_none() {
            return Err(ParseError::new(
                span,
                format!("`{}` is not a module", entry.id),
            ));
        }
        Ok(ModuleUse { module: id, count })
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Result<Token, ParseError> {
        match self.advance() {
            Some(token) if token.kind == kind => Ok(token),
            Some(token) => Err(ParseError::new(token.span, message)),
            None => Err(self.error_at_end(message)),
        }
    }

    fn expect_end(&self) -> Result<(), ParseError> {
        match self.peek() {
            Some(token) => Err(ParseError::new(
                token.span,
                format!("unexpected {}", token.kind.describe()),
            )),
            None => Ok(()),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn follow(&self) -> Follow {
        match self.peek().map(|token| &token.kind) {
            Some(TokenKind::At) => Follow::At,
            Some(TokenKind::Belt { tier, stacked }) => Follow::Belt {
                tier: *tier,
                stacked: *stacked,
            },
            Some(TokenKind::Time(unit)) => Follow::Time(*unit),
            Some(TokenKind::Joule) => Follow::Joule,
            Some(TokenKind::Watt) => Follow::Watt,
            Some(TokenKind::Craft) => Follow::Craft,
            Some(TokenKind::Prototype(id)) => Follow::Prototype(*id, self.peek().unwrap().span),
            _ => Follow::None,
        }
    }

    fn spec_step(&self) -> SpecStep {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Making,
                ..
            }) => SpecStep::Making,
            Some(Token {
                kind: TokenKind::Number(count),
                span,
            }) => SpecStep::Count(*count, *span),
            Some(Token {
                kind: TokenKind::Prototype(id),
                span,
            }) => SpecStep::Prototype(*id, *span),
            _ => SpecStep::Done,
        }
    }

    fn infix(&self) -> (Infix, Span) {
        match self.peek() {
            Some(token) => {
                let infix = match token.kind {
                    TokenKind::In => Infix::In,
                    TokenKind::Plus => Infix::Add,
                    TokenKind::Minus => Infix::Sub,
                    TokenKind::Star => Infix::Mul,
                    TokenKind::Slash => Infix::Div,
                    TokenKind::Caret => Infix::Pow,
                    _ => Infix::None,
                };
                (infix, token.span)
            }
            None => (Infix::None, Span::new(0, 0)),
        }
    }

    fn last_end(&self) -> u32 {
        self.tokens.last().map(|token| token.span.end).unwrap_or(0)
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
    None,
    In,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, Copy)]
enum Follow {
    None,
    At,
    Belt { tier: BeltTier, stacked: bool },
    Time(TimeUnit),
    Joule,
    Watt,
    Craft,
    Prototype(EntryId, Span),
}

#[derive(Debug, Clone, Copy)]
enum SpecStep {
    Done,
    Making,
    Count(f64, Span),
    Prototype(EntryId, Span),
}

fn positive_integer(value: f64, span: Span, message: &str) -> Result<(), ParseError> {
    if value.is_finite() && value > 0.0 && value.fract() == 0.0 {
        Ok(())
    } else {
        Err(ParseError::new(span, message))
    }
}
