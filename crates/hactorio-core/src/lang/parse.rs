//! The parser (§4.1–§4.3) and the document structure (§2.1–§2.2).

use hactorio_data::Registry;

use super::lex::{Keyword, Signal, Token, Unit, is_reserved_word, lex};
use super::value::{Comparison, Quality, ResourceRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub span: Span,
    pub message: String,
}

impl ParseError {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

/// A named property, which is `.name` (§6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Property {
    Rate,
    Capacity,
    Machines,
    Crafts,
    Productivity,
    Pollution,
    Power,
    Energy,
    Abs,
    Level,
    Multiplier,
    In,
    Out,
    Net,
}

impl Property {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "rate" => Property::Rate,
            "capacity" => Property::Capacity,
            "machines" => Property::Machines,
            "crafts" => Property::Crafts,
            "productivity" | "prod" => Property::Productivity,
            "pollution" => Property::Pollution,
            "power" => Property::Power,
            "energy" => Property::Energy,
            "abs" => Property::Abs,
            "level" => Property::Level,
            "multiplier" => Property::Multiplier,
            "in" => Property::In,
            "out" => Property::Out,
            "net" => Property::Net,
            _ => return None,
        })
    }
}

/// The right operand of `.` (§6.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selector {
    Property(Property),
    Signal(Signal),
    Resource(ResourceRef),
}

/// What follows a `.` (§6.6): a known selector, or any expression naming a
/// resource.
#[derive(Debug, Clone, PartialEq)]
enum ParsedSelector {
    Known(Selector),
    Expression(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Compare(Comparison),
    Make,
    In,
    Into,
    From,
}

impl BinaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Pow => "^",
            BinaryOp::Compare(comparison) => comparison.symbol(),
            BinaryOp::Make => "making",
            BinaryOp::In => "in",
            BinaryOp::Into => "into",
            BinaryOp::From => "from",
        }
    }
}

/// One expression node, with the span it occupies (§8).
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

impl Expr {
    pub fn new(span: Span, kind: ExprKind) -> Self {
        Self { span, kind }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Number(f64),
    Unit(Unit),
    Token(hactorio_data::EntryId),
    Qualified {
        id: hactorio_data::EntryId,
        quality: Quality,
    },
    Quality(Quality),
    Variable(String),
    Negate(Box<Expr>),
    StackSize(Box<Expr>),
    StackedBelt(Box<Expr>),
    Recycling(Box<Expr>),
    Select {
        value: Box<Expr>,
        selector: Selector,
    },
    /// `PROCESS . EXPR`, where the expression names the resource to select.
    SelectBy {
        value: Box<Expr>,
        selector: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// One n-ary chain, folded right to left (§4.1).
    Juxtapose(Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    Add,
    Sub,
    Mul,
    Div,
}

impl AssignOp {
    pub fn binary(self) -> Option<BinaryOp> {
        Some(match self {
            AssignOp::Assign => return None,
            AssignOp::Add => BinaryOp::Add,
            AssignOp::Sub => BinaryOp::Sub,
            AssignOp::Mul => BinaryOp::Mul,
            AssignOp::Div => BinaryOp::Div,
        })
    }
}

/// A definition target: a name, optionally followed by one selector (§2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub name: String,
    pub selector: Option<Selector>,
}

fn reserved_target(name: &str, span: Span) -> ParseError {
    ParseError::new(
        span,
        format!("`{name}` is a reserved word and cannot be a variable name"),
    )
}

/// One parsed line, before loops are assembled (§2.1).
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Blank,
    Comment,
    Expression(Expr),
    Definition {
        place: Place,
        op: AssignOp,
        value: Expr,
    },
    LoopStart(Expr),
    LoopEnd,
}

/// The span of a definition's right-hand side, which the completion of a
/// selector after a dot needs (§2.2).
pub fn definition_value_span(line: &str, registry: &Registry) -> Option<Span> {
    let tokens = super::lex::lex(line, registry).ok()?;
    let assign = tokens.iter().position(|(token, _)| {
        matches!(
            token,
            Token::Assign
                | Token::AddAssign
                | Token::SubAssign
                | Token::MulAssign
                | Token::DivAssign
        )
    })?;
    let (_, first) = tokens.get(assign + 1)?;
    let (_, last) = tokens.last()?;
    Some(Span::new(first.start, last.end))
}

pub fn parse_line(line: &str, registry: &Registry) -> Result<Line, ParseError> {
    let tokens = lex(line, registry)?;
    if tokens.is_empty() {
        return Ok(if line.trim_start().starts_with('#') {
            Line::Comment
        } else {
            Line::Blank
        });
    }

    let mut parser = Parser {
        tokens,
        pos: 0,
        registry,
    };

    if matches!(parser.peek(), Some(Token::RBrace)) {
        parser.pos += 1;
        parser.expect_end()?;
        return Ok(Line::LoopEnd);
    }

    if matches!(parser.peek(), Some(Token::Keyword(Keyword::Loop))) {
        parser.pos += 1;
        let count = parser.parse_expr()?;
        parser.expect(Token::LBrace, "expected `{` after the loop count")?;
        parser.expect_end()?;
        return Ok(Line::LoopStart(count));
    }

    if let Some(place) = parser.try_place()? {
        let op = match parser.advance() {
            Some(Token::Assign) => AssignOp::Assign,
            Some(Token::AddAssign) => AssignOp::Add,
            Some(Token::SubAssign) => AssignOp::Sub,
            Some(Token::MulAssign) => AssignOp::Mul,
            Some(Token::DivAssign) => AssignOp::Div,
            _ => unreachable!("try_place only matches assign operators"),
        };
        let value = parser.parse_expr()?;
        parser.expect_end()?;
        return Ok(Line::Definition { place, op, value });
    }

    let expr = parser.parse_expr()?;
    parser.expect_end()?;
    Ok(Line::Expression(expr))
}

// Juxtaposition is handled inside `parse_chain`, so it needs no binding power.
const BP_IN: u8 = 10;
const BP_MAKE: u8 = 20;
const BP_COMPARE: u8 = 30;
const BP_ADD: u8 = 40;
const BP_SUB: u8 = 45;
const BP_MUL: u8 = 50;
/// Unary minus binds between `^` and `/`, so `-2^2` is `-(2^2)` (§4.3).
const BP_NEG: u8 = 57;
const BP_DIV: u8 = 55;
const BP_POW: u8 = 60;

struct Parser<'a> {
    tokens: Vec<(Token, Span)>,
    pos: usize,
    registry: &'a Registry,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(token, _)| token)
    }

    /// The span of the token the parser is looking at, or the end of the line.
    fn peek_span(&self) -> Span {
        match self.tokens.get(self.pos) {
            Some((_, span)) => *span,
            None => {
                let end = self.tokens.last().map_or(0, |(_, span)| span.end);
                Span::new(end, end)
            }
        }
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).map(|(token, _)| token.clone());
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, kind: Token, message: &str) -> Result<(), ParseError> {
        match self.peek() {
            Some(token) if *token == kind => {
                self.pos += 1;
                Ok(())
            }
            Some(token) => Err(ParseError::new(
                self.peek_span(),
                format!("{message}, found {}", token.describe()),
            )),
            None => Err(ParseError::new(self.peek_span(), message)),
        }
    }

    fn expect_end(&mut self) -> Result<(), ParseError> {
        match self.peek() {
            None => Ok(()),
            Some(token) => Err(ParseError::new(
                self.peek_span(),
                format!("unexpected {}", token.describe()),
            )),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_bp(0)
    }

    fn parse_bp(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_chain()?;
        loop {
            // `.` is postfix and binds looser than juxtaposition, so it is
            // applied to the whole chain (§4.3).
            if matches!(self.peek(), Some(Token::Dot)) {
                self.pos += 1;
                lhs = self.selector_expr(lhs)?;
                continue;
            }

            let Some((op, lbp, right_associative)) = self.peek_infix() else {
                break;
            };
            if lbp < min_bp {
                break;
            }
            self.pos += 1;

            let rhs = match op {
                BinaryOp::In | BinaryOp::Into | BinaryOp::From => self.parse_machine_expr()?,
                _ => self.parse_bp(if right_associative { lbp } else { lbp + 1 })?,
            };
            let span = Span::new(lhs.span.start, rhs.span.end);
            lhs = Expr::new(
                span,
                ExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            );
        }
        Ok(lhs)
    }

    fn peek_infix(&self) -> Option<(BinaryOp, u8, bool)> {
        let token = self.peek()?;
        Some(match token {
            Token::Keyword(Keyword::In) => (BinaryOp::In, BP_IN, false),
            Token::Keyword(Keyword::Making) => (BinaryOp::Make, BP_MAKE, false),
            Token::Keyword(Keyword::Into) => (BinaryOp::Into, BP_IN, false),
            Token::Keyword(Keyword::From) => (BinaryOp::From, BP_IN, false),
            Token::Less => (BinaryOp::Compare(Comparison::Less), BP_COMPARE, false),
            Token::Greater => (BinaryOp::Compare(Comparison::Greater), BP_COMPARE, false),
            Token::LessEqual => (BinaryOp::Compare(Comparison::LessEqual), BP_COMPARE, false),
            Token::GreaterEqual => (
                BinaryOp::Compare(Comparison::GreaterEqual),
                BP_COMPARE,
                false,
            ),
            Token::EqualEqual => (BinaryOp::Compare(Comparison::Equal), BP_COMPARE, false),
            Token::NotEqual => (BinaryOp::Compare(Comparison::NotEqual), BP_COMPARE, false),
            Token::Plus => (BinaryOp::Add, BP_ADD, false),
            Token::Minus => (BinaryOp::Sub, BP_SUB, false),
            Token::Star => (BinaryOp::Mul, BP_MUL, false),
            Token::Slash => (BinaryOp::Div, BP_DIV, false),
            Token::Caret => (BinaryOp::Pow, BP_POW, true),
            _ => return None,
        })
    }

    /// An operand chain (§4.1).
    fn parse_chain(&mut self) -> Result<Expr, ParseError> {
        let mut operands = vec![self.parse_operand()?];
        while self.peek().is_some_and(Token::starts_operand) {
            operands.push(self.parse_operand()?);
        }
        Ok(if operands.len() == 1 {
            operands.pop().expect("one operand")
        } else {
            let span = Span::new(
                operands.first().expect("one operand").span.start,
                operands.last().expect("one operand").span.end,
            );
            Expr::new(span, ExprKind::Juxtapose(operands))
        })
    }

    /// The right side of `in`/`into`/`from`: everything tighter than those
    /// operators (§4.3), which includes `making`.
    fn parse_machine_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_bp(BP_IN + 1)
    }

    fn parse_operand(&mut self) -> Result<Expr, ParseError> {
        let span = self.peek_span();
        match self.peek() {
            Some(Token::Minus) => {
                self.pos += 1;
                let inner = self.parse_bp(BP_NEG)?;
                let span = Span::new(span.start, inner.span.end);
                Ok(Expr::new(span, ExprKind::Negate(Box::new(inner))))
            }
            Some(Token::Signal(Signal::StackSize)) => {
                self.pos += 1;
                let inner = self.parse_primary()?;
                let span = Span::new(span.start, inner.span.end);
                Ok(Expr::new(span, ExprKind::StackSize(Box::new(inner))))
            }
            Some(Token::Signal(Signal::StackedBelt)) => {
                self.pos += 1;
                let inner = self.parse_primary()?;
                let span = Span::new(span.start, inner.span.end);
                Ok(Expr::new(span, ExprKind::StackedBelt(Box::new(inner))))
            }
            Some(Token::Signal(Signal::Recycling)) => {
                self.pos += 1;
                let inner = self.parse_primary()?;
                let span = Span::new(span.start, inner.span.end);
                Ok(Expr::new(span, ExprKind::Recycling(Box::new(inner))))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let span = self.peek_span();
        match self.advance() {
            Some(Token::Number(value)) => Ok(Expr::new(span, ExprKind::Number(value))),
            Some(Token::Unit(unit)) => Ok(Expr::new(span, ExprKind::Unit(unit))),
            Some(Token::Prototype(id)) => Ok(Expr::new(span, ExprKind::Token(id))),
            Some(Token::Qualified { id, quality }) => {
                Ok(Expr::new(span, ExprKind::Qualified { id, quality }))
            }
            Some(Token::Quality(quality)) => Ok(Expr::new(span, ExprKind::Quality(quality))),
            Some(Token::Ident(name)) => Ok(Expr::new(span, ExprKind::Variable(name))),
            Some(Token::LParen) => {
                // Parentheses group; they are not part of the expression's span.
                let inner = self.parse_expr()?;
                self.expect(Token::RParen, "expected `)`")?;
                Ok(inner)
            }
            Some(token) => Err(ParseError::new(
                self.peek_span(),
                format!("expected an expression, found {}", token.describe()),
            )),
            None => Err(ParseError::new(self.peek_span(), "expected an expression")),
        }
    }

    fn selector_expr(&mut self, value: Expr) -> Result<Expr, ParseError> {
        let (selector, end) = self.parse_selector()?;
        let span = Span::new(value.span.start, end.end);
        Ok(Expr::new(
            span,
            match selector {
                ParsedSelector::Known(selector) => ExprKind::Select {
                    value: Box::new(value),
                    selector,
                },
                ParsedSelector::Expression(selector) => ExprKind::SelectBy {
                    value: Box::new(value),
                    selector,
                },
            },
        ))
    }

    fn parse_selector(&mut self) -> Result<(ParsedSelector, Span), ParseError> {
        let span = self.peek_span();
        match self.advance() {
            // A known property name is a property; any other name is an
            // expression naming a resource, so `p . q` selects by `q`.
            Some(Token::Ident(name)) => Ok((
                match Property::from_name(&name) {
                    Some(property) => ParsedSelector::Known(Selector::Property(property)),
                    None => ParsedSelector::Expression(Box::new(Expr::new(
                        span,
                        ExprKind::Variable(name),
                    ))),
                },
                span,
            )),
            // `in` is a keyword and also a process property.
            Some(Token::Keyword(Keyword::In)) => Ok((
                ParsedSelector::Known(Selector::Property(Property::In)),
                span,
            )),
            Some(Token::Signal(signal)) => {
                Ok((ParsedSelector::Known(Selector::Signal(signal)), span))
            }
            Some(Token::Prototype(id)) => Ok((
                ParsedSelector::Known(Selector::Resource(self.resource_ref(id, None))),
                span,
            )),
            Some(Token::Qualified { id, quality }) => Ok((
                ParsedSelector::Known(Selector::Resource(self.resource_ref(id, Some(quality)))),
                span,
            )),
            Some(Token::LParen) => {
                let inner = self.parse_expr()?;
                let close = self.peek_span();
                self.expect(Token::RParen, "expected `)`")?;
                Ok((
                    ParsedSelector::Expression(Box::new(inner)),
                    Span::new(span.start, close.end),
                ))
            }
            Some(token) => Err(ParseError::new(
                span,
                format!("expected a property after `.`, found {}", token.describe()),
            )),
            None => Err(ParseError::new(span, "expected a property after `.`")),
        }
    }

    fn resource_ref(&self, id: hactorio_data::EntryId, quality: Option<Quality>) -> ResourceRef {
        let entry = self.registry.entry_by_index(id);
        ResourceRef {
            id,
            is_fluid: entry.fluid,
            quality: if entry.fluid { None } else { quality },
        }
    }

    /// Recognises `NAME ASSIGNOP` and `NAME . selector ASSIGNOP` (§2.2).
    fn try_place(&mut self) -> Result<Option<Place>, ParseError> {
        let Some(Token::Ident(name)) = self.peek() else {
            return Ok(None);
        };
        let name = name.clone();
        let name_span = self.tokens[self.pos].1;
        let after = self.tokens.get(self.pos + 1).map(|(token, _)| token);
        let is_assign = |token: Option<&Token>| {
            matches!(
                token,
                Some(Token::Assign)
                    | Some(Token::AddAssign)
                    | Some(Token::SubAssign)
                    | Some(Token::MulAssign)
                    | Some(Token::DivAssign)
            )
        };
        if is_assign(after) {
            if is_reserved_word(&name) {
                return Err(reserved_target(&name, name_span));
            }
            self.pos += 1;
            return Ok(Some(Place {
                name,
                selector: None,
            }));
        }
        if matches!(after, Some(Token::Dot)) {
            let saved = self.pos;
            self.pos += 2;
            let (selector, _) = self.parse_selector()?;
            let ParsedSelector::Known(selector) = selector else {
                self.pos = saved;
                return Ok(None);
            };
            if is_assign(self.peek()) {
                if is_reserved_word(&name) {
                    return Err(reserved_target(&name, name_span));
                }
                return Ok(Some(Place {
                    name,
                    selector: Some(selector),
                }));
            }
            self.pos = saved;
            return Ok(None);
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::test_registry;

    fn expr(line: &str) -> Expr {
        match parse_line(line, &test_registry()).expect("parses") {
            Line::Expression(expr) => expr,
            other => panic!("expected an expression, got {other:?}"),
        }
    }

    /// Renders the tree's shape so grouping assertions stay readable.
    fn shape(expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Number(value) => format!("{value}"),
            ExprKind::Unit(_) => "unit".to_string(),
            ExprKind::Token(_) => "token".to_string(),
            ExprKind::Qualified { .. } => "qualified".to_string(),
            ExprKind::Quality(quality) => format!(":{}:", quality.name()),
            ExprKind::Variable(name) => name.clone(),
            ExprKind::Negate(inner) => format!("(-{})", shape(inner)),
            ExprKind::StackSize(inner) => format!("(stack-size {})", shape(inner)),
            ExprKind::StackedBelt(inner) => format!("(stacked-belt {})", shape(inner)),
            ExprKind::Recycling(inner) => format!("(recycling {})", shape(inner)),
            ExprKind::Select { value, selector } => format!("({}.{:?})", shape(value), selector),
            ExprKind::SelectBy { value, selector } => {
                format!("({}.expr {})", shape(value), shape(selector))
            }
            ExprKind::Binary { op, lhs, rhs } => {
                format!("({} {} {})", shape(lhs), op.symbol(), shape(rhs))
            }
            ExprKind::Juxtapose(operands) => {
                let parts: Vec<String> = operands.iter().map(shape).collect();
                format!("[{}]", parts.join(" "))
            }
        }
    }

    #[test]
    fn juxtaposition_is_one_chain() {
        assert_eq!(
            shape(&expr(":beacon: 2 :speed-module-2:")),
            "[token 2 token]"
        );
    }

    #[test]
    fn division_binds_tighter_than_multiplication() {
        assert_eq!(shape(&expr("3 * 1 / 2")), "(3 * (1 / 2))");
    }

    #[test]
    fn minus_binds_tighter_than_plus() {
        assert_eq!(shape(&expr("1 + 2 - 3")), "(1 + (2 - 3))");
        assert_eq!(shape(&expr("1 - 2 + 3")), "((1 - 2) + 3)");
    }

    #[test]
    fn unary_minus_binds_between_multiplication_and_power() {
        assert_eq!(shape(&expr("-2 ^ 2")), "(-(2 ^ 2))");
        assert_eq!(shape(&expr("-2 * 3")), "((-2) * 3)");
    }

    #[test]
    fn power_is_right_associative() {
        assert_eq!(shape(&expr("2 ^ 3 ^ 2")), "(2 ^ (3 ^ 2))");
    }

    #[test]
    fn dot_applies_to_the_whole_chain() {
        assert_eq!(shape(&expr("a b .rate")), "([a b].Property(Rate))");
    }

    #[test]
    fn making_is_looser_than_arithmetic_and_tighter_than_in() {
        assert_eq!(
            shape(&expr("2 * :foundry: making :iron-plate:")),
            "((2 * token) making token)"
        );
        assert_eq!(
            shape(&expr(":foundry: making :iron-plate: * 2")),
            "(token making (token * 2))"
        );
        assert_eq!(
            shape(&expr("3 :iron-plate: in :foundry: making :iron-plate:")),
            "([3 token] in (token making token))"
        );
    }

    #[test]
    fn in_takes_everything_tighter_than_in_on_its_right() {
        assert_eq!(
            shape(&expr("3 :iron-plate: in :foundry: * 2")),
            "([3 token] in (token * 2))"
        );
    }

    #[test]
    fn parse_errors_point_at_the_offending_token() {
        // A missing operand points at the end of the line.
        let error = parse_line("1 +", &test_registry()).unwrap_err();
        assert_eq!(error.span, Span::new(3, 3));

        // An unexpected token points at that token.
        let error = parse_line("1 2 )", &test_registry()).unwrap_err();
        assert_eq!(error.span, Span::new(4, 5));

        // A selector that cannot name a resource points at what followed the
        // dot; any name is allowed, because a name can be a variable.
        let error = parse_line("x.+", &test_registry()).unwrap_err();
        assert_eq!(error.span, Span::new(2, 3));
        assert!(parse_line("x.nope", &test_registry()).is_ok());
    }

    #[test]
    fn definitions_are_statements() {
        match parse_line("q = :rare:", &test_registry()).unwrap() {
            Line::Definition { place, op, .. } => {
                assert_eq!(place.name, "q");
                assert_eq!(op, AssignOp::Assign);
                assert!(place.selector.is_none());
            }
            other => panic!("expected a definition, got {other:?}"),
        }
    }

    #[test]
    fn reserved_words_are_not_definition_targets() {
        for line in ["s = 1", "kJ += 1", "loop -= 1", "making = 1", "in = 1"] {
            assert!(
                parse_line(line, &test_registry()).is_err(),
                "{line} parsed as a definition"
            );
        }
    }

    #[test]
    fn assignment_is_not_an_expression() {
        assert!(parse_line("1 = 2", &test_registry()).is_err());
        assert!(parse_line("(q = 1)", &test_registry()).is_err());
        assert!(parse_line("q = 1", &test_registry()).is_ok());
    }
}
