//! Parsed statements.

use hactorio_data::{EntryId, RecipeId};

use crate::belt::BeltTier;
use crate::value::Value;

/// A byte range within one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }
}

/// One parsed line.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    /// The span of the statement within the line, excluding comments.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Blank,
    Comment,
    Expression(Expr),
    Definition { name: String, value: Expr },
}

/// An expression with prototype tokens already resolved against the registry.
///
/// Parsed expressions carry no evaluated coefficients: belt stacking and every
/// other setting-dependent effect is applied during evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    Belt {
        count: f64,
        tier: BeltTier,
        stacked: bool,
        item: EntryId,
    },
    Machine(MachineSpec),
    Neg(Box<Expr>),
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    In {
        quantity: Box<Expr>,
        machine: MachineSpec,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// `count? '@' MACHINE module* (making RECIPE)?`
#[derive(Debug, Clone, PartialEq)]
pub struct MachineSpec {
    pub machine: EntryId,
    /// Positive integer; defaults to 1.
    pub count: f64,
    pub modules: Vec<ModuleUse>,
    pub recipe: Option<RecipeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleUse {
    pub module: EntryId,
    /// Positive integer; defaults to 1.
    pub count: f64,
}
