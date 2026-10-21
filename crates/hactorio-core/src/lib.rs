//! Units, parser, evaluator, and settings for hactorio.
//!
//! This crate has no dependency on egui and is testable headlessly.

mod ast;
mod belt;
mod config;
mod eval;
mod format;
mod lexer;
mod line_cache;
mod parser;
mod settings;
mod text;
mod value;

pub use ast::{BinaryOp, Expr, MachineSpec, ModuleUse, Span, Statement, StatementKind};
pub use belt::{BeltTier, belt_capacity, belt_value, stack_size};
pub use eval::{
    Eval, EvalError, LineOutput, MachineOutput, MachineValue, ModuleCount, Output, RecipeOutput,
    RecipeQuantity, RecipeValue, StatementOutput, evaluate,
};
pub use format::format_value;
pub use line_cache::LineCache;
pub use parser::{ParseError, parse_line};
pub use settings::{ItemRateDisplay, Settings, TimeUnit};
pub use text::{from_plaintext, to_plaintext};
pub use value::{BaseUnit, Value, ValueError};
